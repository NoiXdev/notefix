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
//!   2. Reading the wrapped DEK back out of the keychain ([`load_dek`]).
//!
//! The DEK is stored base64-encoded under the keychain account `vault-dek`
//! (service `dev.noix.notefix`, shared with the auth-token storage in
//! `auth.rs`). Note that the keychain entry itself carries no biometric ACL —
//! the Touch ID gate is our explicit [`authenticate`] call, a separate step
//! from reading the entry. The DEK is never logged, and every transient
//! base64/byte buffer is zeroized after use.

use crate::vault::aead::Dek;
use crate::vault::VaultError;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use zeroize::Zeroize;

/// Keychain service string — shared with `auth.rs` (`KEYCHAIN_SERVICE`).
const KEYCHAIN_SERVICE: &str = "dev.noix.notefix";
/// Keychain account holding the base64-encoded biometric-wrapped DEK.
const BIOMETRIC_ACCOUNT: &str = "vault-dek";

fn biometric_entry() -> Result<keyring::Entry, VaultError> {
    keyring::Entry::new(KEYCHAIN_SERVICE, BIOMETRIC_ACCOUNT)
        .map_err(|e| VaultError::Io(e.to_string()))
}

/// Base64-encode a DEK for keychain storage. The returned `String` holds key
/// material — callers must zeroize it once written.
fn encode_dek(dek: &Dek) -> String {
    STANDARD.encode(dek.expose())
}

/// Decode a base64 DEK read back from the keychain, zeroizing transient byte
/// buffers on every path. Wrong length ⇒ [`VaultError::Corrupt`]; invalid
/// base64 ⇒ [`VaultError::Crypto`].
fn decode_dek(b64: &str) -> Result<Dek, VaultError> {
    let mut bytes = STANDARD
        .decode(b64)
        .map_err(|e| VaultError::Crypto(e.to_string()))?;
    let mut arr: [u8; 32] = match bytes.as_slice().try_into() {
        Ok(a) => a,
        Err(_) => {
            bytes.zeroize();
            return Err(VaultError::Corrupt);
        }
    };
    let dek = Dek::from_bytes(arr);
    arr.zeroize();
    bytes.zeroize();
    Ok(dek)
}

/// Map the result of a keychain `set_password` call to our domain error.
/// Pulled out of [`store_dek`] so this bookkeeping is testable without a real
/// keychain write.
fn map_store_result(res: Result<(), keyring::Error>) -> Result<(), VaultError> {
    res.map_err(|e| VaultError::Io(e.to_string()))
}

/// Writes the base64-encoded DEK to the `vault-dek` keychain entry.
pub fn store_dek(dek: &Dek) -> Result<(), VaultError> {
    let entry = biometric_entry()?;
    let mut b64 = encode_dek(dek);
    let res = map_store_result(entry.set_password(&b64));
    b64.zeroize();
    res
}

/// Map the result of a keychain `get_password` call to the `load_dek`
/// outcome: no entry ⇒ `Ok(None)`, other keychain error ⇒ `Err(Io)`, entry
/// found ⇒ base64-decode it. Pulled out of [`load_dek`] so this mapping and
/// decoding logic is testable without a real keychain read.
fn map_load_result(res: Result<String, keyring::Error>) -> Result<Option<Dek>, VaultError> {
    let mut b64 = match res {
        Ok(s) => s,
        Err(keyring::Error::NoEntry) => return Ok(None),
        Err(e) => return Err(VaultError::Io(e.to_string())),
    };
    let dek = decode_dek(&b64);
    b64.zeroize();
    dek.map(Some)
}

/// Reads and base64-decodes the `vault-dek` keychain entry. Returns
/// `Ok(None)` when no entry exists (biometric unlock not enrolled).
pub fn load_dek() -> Result<Option<Dek>, VaultError> {
    let entry = biometric_entry()?;
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
pub fn clear() -> Result<(), VaultError> {
    map_clear_result(biometric_entry()?.delete_credential())
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
pub fn is_enrolled() -> bool {
    match biometric_entry() {
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
    fn encode_decode_roundtrips() {
        let dek = Dek::random();
        let encoded = encode_dek(&dek);
        let decoded = decode_dek(&encoded).unwrap();
        assert_eq!(dek.expose(), decoded.expose());
    }

    #[test]
    fn decode_rejects_wrong_length() {
        // `Dek` has no `Debug`/`PartialEq` by design (key material), so match
        // on the error rather than `assert_eq!`-ing the whole `Result`.
        let short = STANDARD.encode([0u8; 16]);
        assert!(matches!(decode_dek(&short), Err(VaultError::Corrupt)));
    }

    #[test]
    fn decode_rejects_invalid_base64() {
        assert!(matches!(
            decode_dek("not valid base64 !!!"),
            Err(VaultError::Crypto(_))
        ));
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
        let encoded = encode_dek(&dek);
        let out = map_load_result(Ok(encoded)).unwrap();
        assert_eq!(out.unwrap().expose(), dek.expose());
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
