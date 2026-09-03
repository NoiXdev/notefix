//! Biometric unlock for the protected-notes vault.
//!
//! **v1 scope (controller decision): desktop macOS (Touch ID / device-owner
//! authentication) plus the OS keychain, only.** No mobile biometric plugin is
//! wired up — the mobile app is a prototype and pulling in a mobile plugin
//! risks the desktop build. On non-macOS desktop and on mobile,
//! [`is_available`] returns `false` and [`authenticate`] returns
//! [`VaultError::Unsupported`].
//!
//! The unlock flow is two independent steps:
//!   1. A Touch ID prompt via `LAContext.evaluatePolicy` ([`authenticate`]).
//!   2. Reading the wrapped ring back out of the keychain ([`load_ring`]).
//!
//! The keychain item carries the WHOLE key ring, not just one generation: a
//! rotated workspace needs every generation still in play to open its older
//! notes, and a single-generation item left every note sealed under another
//! generation unreadable after a Touch ID unlock. The ring is stored
//! base64-encoded under the keychain account `vault-dek` (service
//! `dev.noix.notefix`, shared with the auth-token storage in `auth.rs`). Note
//! that the keychain entry itself carries no biometric ACL — the Touch ID
//! gate is our explicit [`authenticate`] call, a separate step from reading
//! the entry. The DEKs are never logged, and every transient base64/byte
//! buffer is zeroized after use.

use crate::vault::aead::Dek;
use crate::vault::VaultError;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use zeroize::Zeroize;

/// Keychain service string — shared with `auth.rs` (`KEYCHAIN_SERVICE`).
const KEYCHAIN_SERVICE: &str = "dev.noix.notefix";
/// Keychain account prefix for the biometric-wrapped DEK. The item is scoped
/// PER CONTEXT (`vault-dek:<context id>`): every context is its own vault
/// with its own DEK, so a single app-wide item would hand context B the DEK
/// of context A — reported as "enrolled" everywhere, and able to seal notes
/// under a key B's record can never unwrap.
const BIOMETRIC_ACCOUNT_PREFIX: &str = "vault-dek";
/// The pre-per-context item name. Never read as an enrollment any more —
/// it can't be attributed to a context — and deleted once at startup.
const LEGACY_BIOMETRIC_ACCOUNT: &str = "vault-dek";

/// Keychain account name for a context's biometric DEK.
pub fn account_for(context_id: &str) -> String {
    format!("{BIOMETRIC_ACCOUNT_PREFIX}:{context_id}")
}

fn biometric_entry(context_id: &str) -> Result<keyring::Entry, VaultError> {
    keyring::Entry::new(KEYCHAIN_SERVICE, &account_for(context_id))
        .map_err(|e| VaultError::Io(e.to_string()))
}

/// Deletes the legacy app-wide `vault-dek` item (idempotent). Called once at
/// startup so a stale, context-less DEK can never be picked up again.
pub fn clear_legacy() -> Result<(), VaultError> {
    let entry = keyring::Entry::new(KEYCHAIN_SERVICE, LEGACY_BIOMETRIC_ACCOUNT)
        .map_err(|e| VaultError::Io(e.to_string()))?;
    map_clear_result(entry.delete_credential())
}

/// Payload version byte for a whole-ring keychain item.
const RING_FORMAT_V2: u8 = 2;
const ENTRY_LEN: usize = 36;

/// Base64 of `0x02`, the generation count, then per generation four
/// big-endian generation bytes and the 32 key bytes. The ring travels whole
/// because a rotated workspace needs every generation to open its older
/// notes; a single-generation item (as this module stored before) left every
/// note sealed under another generation unreadable after a Touch ID unlock.
/// Callers zeroize the returned string once written.
fn encode_ring(ring: &[(u32, Dek)]) -> String {
    let mut bytes = Vec::with_capacity(2 + ring.len() * ENTRY_LEN);
    bytes.push(RING_FORMAT_V2);
    bytes.push(ring.len() as u8);
    for (generation, dek) in ring {
        bytes.extend_from_slice(&generation.to_be_bytes());
        bytes.extend_from_slice(dek.expose());
    }
    let b64 = STANDARD.encode(&bytes);
    bytes.zeroize();
    b64
}

/// `encode_ring` with the invariants the decoder enforces: 1..=255
/// generations, strictly ascending.
fn encode_ring_checked(ring: &[(u32, Dek)]) -> Result<String, VaultError> {
    if ring.is_empty() || ring.len() > u8::MAX as usize {
        return Err(VaultError::Corrupt);
    }
    if ring.windows(2).any(|w| w[0].0 >= w[1].0) {
        return Err(VaultError::Corrupt);
    }
    Ok(encode_ring(ring))
}

fn dek_from(key: &[u8]) -> Result<Dek, VaultError> {
    let mut arr: [u8; 32] = key.try_into().map_err(|_| VaultError::Corrupt)?;
    let dek = Dek::from_bytes(arr);
    arr.zeroize();
    Ok(dek)
}

/// Decodes a keychain payload: v2 rings, plus the two legacy shapes — a bare
/// 32-byte DEK (generation 1) and a 36-byte generation-tagged DEK.
fn decode_ring_bytes(bytes: &[u8]) -> Result<Vec<(u32, Dek)>, VaultError> {
    match bytes.len() {
        32 => Ok(vec![(1, dek_from(bytes)?)]),
        36 => {
            let generation = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            Ok(vec![(generation, dek_from(&bytes[4..])?)])
        }
        n if n >= 2 && bytes[0] == RING_FORMAT_V2 => {
            let count = bytes[1] as usize;
            if count == 0 || n != 2 + count * ENTRY_LEN {
                return Err(VaultError::Corrupt);
            }
            let mut ring: Vec<(u32, Dek)> = Vec::with_capacity(count);
            for i in 0..count {
                let off = 2 + i * ENTRY_LEN;
                let generation = u32::from_be_bytes([
                    bytes[off],
                    bytes[off + 1],
                    bytes[off + 2],
                    bytes[off + 3],
                ]);
                if ring.last().is_some_and(|(g, _)| *g >= generation) {
                    return Err(VaultError::Corrupt);
                }
                ring.push((generation, dek_from(&bytes[off + 4..off + ENTRY_LEN])?));
            }
            Ok(ring)
        }
        _ => Err(VaultError::Corrupt),
    }
}

fn decode_ring(b64: &str) -> Result<Vec<(u32, Dek)>, VaultError> {
    let mut bytes = STANDARD
        .decode(b64)
        .map_err(|e| VaultError::Crypto(e.to_string()))?;
    let ring = decode_ring_bytes(&bytes);
    bytes.zeroize();
    ring
}

/// Map the result of a keychain `set_password` call to our domain error.
/// Pulled out of [`store_ring`] so this bookkeeping is testable without a
/// real keychain write.
fn map_store_result(res: Result<(), keyring::Error>) -> Result<(), VaultError> {
    res.map_err(|e| VaultError::Io(e.to_string()))
}

/// Writes the whole ring to the context's keychain item.
pub fn store_ring(context_id: &str, ring: &[(u32, Dek)]) -> Result<(), VaultError> {
    let entry = biometric_entry(context_id)?;
    let mut b64 = encode_ring_checked(ring)?;
    let res = map_store_result(entry.set_password(&b64));
    b64.zeroize();
    res
}

/// Map the result of a keychain `get_password` call to the `load_ring`
/// outcome: no entry ⇒ `Ok(None)`, other keychain error ⇒ `Err(Io)`, entry
/// found ⇒ base64-decode it. Pulled out of [`load_ring`] so this mapping and
/// decoding logic is testable without a real keychain read.
fn map_load_result(
    res: Result<String, keyring::Error>,
) -> Result<Option<Vec<(u32, Dek)>>, VaultError> {
    let mut b64 = match res {
        Ok(s) => s,
        Err(keyring::Error::NoEntry) => return Ok(None),
        Err(e) => return Err(VaultError::Io(e.to_string())),
    };
    let ring = decode_ring(&b64);
    b64.zeroize();
    ring.map(Some)
}

/// Reads the ring back. `Ok(None)` when biometric unlock is not enrolled.
pub fn load_ring(context_id: &str) -> Result<Option<Vec<(u32, Dek)>>, VaultError> {
    let entry = biometric_entry(context_id)?;
    map_load_result(entry.get_password())
}

/// Map the result of a keychain `delete_credential` call: success or a
/// missing entry both count as success (disabling an already-disabled
/// biometric unlock is a no-op). Pulled out of [`clear`] so this bookkeeping
/// is testable without a real keychain delete.
fn map_clear_result(res: Result<(), keyring::Error>) -> Result<(), VaultError> {
    match res {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(VaultError::Io(e.to_string())),
    }
}

/// Deletes the `vault-dek` keychain entry. A missing entry is treated as
/// success (disabling an already-disabled biometric unlock is a no-op).
pub fn clear(context_id: &str) -> Result<(), VaultError> {
    map_clear_result(biometric_entry(context_id)?.delete_credential())
}

/// Map the result of a keychain `get_password` call to an enrollment flag,
/// zeroizing the secret either way. Pulled out of [`is_enrolled`] so this
/// bookkeeping is testable without a real keychain read.
fn map_enrolled_result(res: Result<String, keyring::Error>) -> bool {
    match res {
        Ok(mut secret) => {
            secret.zeroize();
            true
        }
        Err(_) => false,
    }
}

/// `true` iff a `vault-dek` entry exists. Used by `vault_status` to report
/// enrollment **without** prompting for Touch ID or exposing the DEK: reading
/// the entry does not prompt (no biometric ACL is set on it), and the fetched
/// base64 secret is zeroized and dropped immediately.
pub fn is_enrolled(context_id: &str) -> bool {
    match biometric_entry(context_id) {
        Ok(entry) => map_enrolled_result(entry.get_password()),
        Err(_) => false,
    }
}

// --- platform-specific: availability + the Touch ID prompt ------------------

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::{NSError, NSString};
    use objc2_local_authentication::{LAContext, LAPolicy};
    use std::sync::mpsc;

    /// Whether this Mac can evaluate biometric (Touch ID) authentication —
    /// i.e. the hardware exists and a fingerprint is enrolled. Performs no UI,
    /// so it is cheap and safe to call from any thread (including the main
    /// thread, where `vault_status` runs).
    pub fn is_available() -> bool {
        let ctx = unsafe { LAContext::new() };
        unsafe { ctx.canEvaluatePolicy_error(LAPolicy::DeviceOwnerAuthenticationWithBiometrics) }
            .is_ok()
    }

    /// Presents the Touch ID / device-owner authentication dialog and blocks
    /// until the user responds. Uses `DeviceOwnerAuthentication` so a failed or
    /// unavailable fingerprint falls back to the Mac login password rather than
    /// locking the user out; cancelling still returns an `Err`, and the vault's
    /// passphrase unlock remains available regardless.
    ///
    /// `evaluatePolicy` is callback-based: the reply block fires on an
    /// arbitrary queue and presents its dialog on the main run loop. This
    /// function blocks on a channel waiting for that reply, so **the caller
    /// must run it off the main thread** or the run loop cannot present the
    /// dialog (`vault_unlock_biometric` offloads via `spawn_blocking`).
    pub fn authenticate(reason: &str) -> Result<(), VaultError> {
        let ctx = unsafe { LAContext::new() };
        let reason = NSString::from_str(reason);
        let (tx, rx) = mpsc::channel::<bool>();
        // The block escapes into the framework and must be 'static + Send; it
        // owns the channel sender (no borrows). A send error just means the
        // receiver is gone, which we cannot do anything about here.
        let block = RcBlock::new(move |success: Bool, _error: *mut NSError| {
            let _ = tx.send(success.as_bool());
        });
        unsafe {
            ctx.evaluatePolicy_localizedReason_reply(
                LAPolicy::DeviceOwnerAuthentication,
                &reason,
                &block,
            );
        }
        match rx.recv() {
            Ok(true) => Ok(()),
            // User cancelled or authentication failed. Surfaced as a generic
            // error the frontend can treat as "fall back to the passphrase".
            Ok(false) => Err(VaultError::Io(
                "biometric authentication cancelled or failed".into(),
            )),
            Err(_) => Err(VaultError::Io("biometric reply channel closed".into())),
        }
    }
}

/// Whether biometric authentication can be evaluated on this device.
#[cfg(target_os = "macos")]
pub fn is_available() -> bool {
    macos::is_available()
}

// TODO(mobile): wire tauri-plugin-biometric when the mobile app is developed.
#[cfg(not(target_os = "macos"))]
pub fn is_available() -> bool {
    false
}

/// Prompt for biometric / device-owner authentication, blocking until the user
/// responds. `Ok(())` on success; `Err` on cancel, failure, or unsupported
/// platform.
#[cfg(target_os = "macos")]
pub fn authenticate(reason: &str) -> Result<(), VaultError> {
    macos::authenticate(reason)
}

// TODO(mobile): wire tauri-plugin-biometric when the mobile app is developed.
#[cfg(not(target_os = "macos"))]
pub fn authenticate(_reason: &str) -> Result<(), VaultError> {
    Err(VaultError::Unsupported)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_round_trips_one_and_three_generations() {
        let one = vec![(1u32, Dek::random())];
        let back = decode_ring(&encode_ring(&one)).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].0, 1);
        assert_eq!(back[0].1.expose(), one[0].1.expose());

        let three = vec![
            (1u32, Dek::random()),
            (2, Dek::random()),
            (5, Dek::random()),
        ];
        let back = decode_ring(&encode_ring(&three)).unwrap();
        assert_eq!(
            back.iter().map(|(g, _)| *g).collect::<Vec<_>>(),
            vec![1, 2, 5]
        );
        for (a, b) in three.iter().zip(back.iter()) {
            assert_eq!(a.1.expose(), b.1.expose());
        }
    }

    #[test]
    fn legacy_items_still_decode() {
        let dek = Dek::random();
        let bare = STANDARD.encode(dek.expose());
        let back = decode_ring(&bare).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].0, 1);
        assert_eq!(back[0].1.expose(), dek.expose());

        let mut tagged = 3u32.to_be_bytes().to_vec();
        tagged.extend_from_slice(dek.expose());
        let back = decode_ring(&STANDARD.encode(&tagged)).unwrap();
        assert_eq!(back[0].0, 3);
    }

    #[test]
    fn malformed_rings_are_rejected_not_panicked() {
        assert!(decode_ring("not base64!").is_err());
        assert!(decode_ring(&STANDARD.encode([2u8, 0])).is_err()); // count 0
        assert!(decode_ring(&STANDARD.encode([2u8, 1, 0, 0])).is_err()); // truncated
        assert!(decode_ring(&STANDARD.encode([9u8, 1])).is_err()); // unknown version
        let mut dup = vec![2u8, 2];
        for _ in 0..2 {
            dup.extend_from_slice(&1u32.to_be_bytes());
            dup.extend_from_slice(Dek::random().expose());
        }
        assert!(decode_ring(&STANDARD.encode(&dup)).is_err()); // not ascending
        assert!(encode_ring_checked(&[]).is_err());
    }

    #[test]
    fn map_store_result_passes_through_ok_and_maps_err() {
        assert!(map_store_result(Ok(())).is_ok());
        assert!(matches!(
            map_store_result(Err(keyring::Error::NoEntry)),
            Err(VaultError::Io(_))
        ));
    }

    #[test]
    fn map_load_result_decodes_the_found_entry() {
        let dek = Dek::random();
        let encoded = encode_ring(&[(3, dek.clone())]);
        let ring = map_load_result(Ok(encoded)).unwrap().unwrap();
        assert_eq!(ring.len(), 1);
        assert_eq!(ring[0].0, 3);
        assert_eq!(ring[0].1.expose(), dek.expose());
    }

    #[test]
    fn map_load_result_no_entry_is_ok_none() {
        assert!(map_load_result(Err(keyring::Error::NoEntry))
            .unwrap()
            .is_none());
    }

    #[test]
    fn map_load_result_other_keychain_error_maps_to_io() {
        let err = keyring::Error::Invalid("account".into(), "bad".into());
        assert!(matches!(map_load_result(Err(err)), Err(VaultError::Io(_))));
    }

    #[test]
    fn map_load_result_propagates_decode_errors() {
        let short = STANDARD.encode([0u8; 16]);
        assert!(matches!(
            map_load_result(Ok(short)),
            Err(VaultError::Corrupt)
        ));
    }

    #[test]
    fn keychain_account_is_scoped_per_context_and_distinct_from_legacy() {
        assert_eq!(account_for("ctx-a"), "vault-dek:ctx-a");
        assert_ne!(account_for("ctx-a"), account_for("ctx-b"));
        assert_ne!(account_for("ctx-a"), LEGACY_BIOMETRIC_ACCOUNT);
    }

    #[test]
    fn map_clear_result_treats_ok_and_no_entry_as_success() {
        assert!(map_clear_result(Ok(())).is_ok());
        assert!(map_clear_result(Err(keyring::Error::NoEntry)).is_ok());
    }

    #[test]
    fn map_clear_result_other_error_maps_to_io() {
        let err = keyring::Error::Invalid("account".into(), "bad".into());
        assert!(matches!(map_clear_result(Err(err)), Err(VaultError::Io(_))));
    }

    #[test]
    fn map_enrolled_result_true_on_found_false_on_error() {
        assert!(map_enrolled_result(Ok("dGVzdA==".to_string())));
        assert!(!map_enrolled_result(Err(keyring::Error::NoEntry)));
    }

    // On any non-macOS target the biometric prompt is unsupported. (This does
    // not run on the macOS host, but documents and enforces the contract for
    // Linux/Windows/mobile builds, where a real prompt is impossible.)
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn authenticate_unsupported_off_macos() {
        assert_eq!(authenticate("reason"), Err(VaultError::Unsupported));
        assert!(!is_available());
    }
}
