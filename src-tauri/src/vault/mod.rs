pub mod aead;
pub mod biometric;
pub mod kdf;
pub mod recovery;
pub mod state;

use aead::Dek;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use kdf::{derive_kek, unwrap_dek, wrap_dek, KdfParams};
use rand::Rng;
use recovery::RecoveryKey;
use serde::{Deserialize, Serialize};

/// Errors produced by the protected-notes vault subsystem.
///
/// This enum is intentionally defined with its full variant set up front
/// (Task 1) even though only a subset is used by this task's code, so that
/// later tasks building on top of `vault` compile without touching this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultError {
    /// A cryptographic operation failed; the string carries a
    /// non-sensitive diagnostic message (never key material).
    Crypto(String),
    /// Decryption/authentication failed, most likely because the wrong key
    /// (or wrong associated data) was used.
    WrongKey,
    /// The stored data is malformed or too short to be a valid sealed blob.
    Corrupt,
    /// An operation was attempted before the vault was unlocked.
    NotUnlocked,
    /// The requested operation or configuration is not supported.
    Unsupported,
    /// An I/O error occurred; the string carries a non-sensitive
    /// diagnostic message.
    Io(String),
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VaultError::Crypto(msg) => write!(f, "vault crypto error: {msg}"),
            VaultError::WrongKey => write!(f, "vault: wrong key or corrupted data"),
            VaultError::Corrupt => write!(f, "vault: corrupt data"),
            VaultError::NotUnlocked => write!(f, "vault: not unlocked"),
            VaultError::Unsupported => write!(f, "vault: unsupported operation"),
            VaultError::Io(msg) => write!(f, "vault I/O error: {msg}"),
        }
    }
}

impl From<VaultError> for String {
    fn from(e: VaultError) -> String {
        e.to_string()
    }
}

/// The on-disk record for a protected-notes vault: everything needed to
/// unlock the vault DEK given either the user's passphrase or their
/// one-time recovery key, but nothing that reveals the DEK on its own.
///
/// Not yet called from app code — wired in by a later task — so items here
/// carry `#[allow(dead_code)]` per the Task 1/2 precedent in this module.
#[allow(dead_code)]
#[derive(Clone)]
pub struct VaultRecord {
    pub kdf_params: KdfParams,
    pub dek_wrapped_pass: Vec<u8>,
    pub recovery_salt: [u8; 16],
    pub dek_wrapped_recovery: Vec<u8>,
    /// A fixed magic sealed under the DEK. Lets every unlock path prove a
    /// candidate DEK really belongs to THIS vault before installing it —
    /// the guard against a DEK from another context (e.g. a biometric
    /// keychain item enrolled elsewhere) sealing notes here with a key this
    /// record can never unwrap again. `None` only on records written before
    /// this field existed; they gain it on the next successful passphrase or
    /// recovery unlock (see `ops::ensure_dek_check`).
    pub dek_check: Option<Vec<u8>>,
}

/// AAD + plaintext of the DEK check blob. Both are constants, so the blob
/// reveals nothing about the DEK; a wrong key simply fails to open it.
const DEK_CHECK_AAD: &[u8] = b"notefix-vault-dek-check";
const DEK_CHECK_MAGIC: &[u8] = b"notefix-vault-ok";

/// Seals the check magic under `dek` (fresh nonce each time).
pub fn make_dek_check(dek: &Dek) -> Vec<u8> {
    aead::seal(dek, DEK_CHECK_AAD, DEK_CHECK_MAGIC)
}

/// Proves `dek` belongs to `rec`. `Ok(true)` = verified, `Ok(false)` = the
/// record predates the check (nothing to verify against), `Err(WrongKey)` =
/// the DEK does NOT belong to this vault.
pub fn verify_dek(rec: &VaultRecord, dek: &Dek) -> Result<bool, VaultError> {
    match &rec.dek_check {
        None => Ok(false),
        Some(blob) => match aead::open(dek, DEK_CHECK_AAD, blob) {
            Ok(pt) if pt == DEK_CHECK_MAGIC => Ok(true),
            _ => Err(VaultError::WrongKey),
        },
    }
}

/// Wire format for [`VaultRecord`]: identical fields, but the raw byte
/// vectors/arrays are base64 strings so the record round-trips through
/// `serde_json` cleanly.
#[derive(Serialize, Deserialize)]
struct VaultRecordJson {
    kdf_params: KdfParams,
    dek_wrapped_pass: String,
    recovery_salt: String,
    dek_wrapped_recovery: String,
    /// Absent in records written before the DEK check existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dek_check: Option<String>,
}

#[allow(dead_code)]
impl VaultRecord {
    /// Serializes this record to a JSON string, base64-encoding the raw
    /// byte fields.
    pub fn to_json(&self) -> String {
        let wire = VaultRecordJson {
            kdf_params: self.kdf_params.clone(),
            dek_wrapped_pass: STANDARD.encode(&self.dek_wrapped_pass),
            recovery_salt: STANDARD.encode(self.recovery_salt),
            dek_wrapped_recovery: STANDARD.encode(&self.dek_wrapped_recovery),
            dek_check: self.dek_check.as_ref().map(|b| STANDARD.encode(b)),
        };
        serde_json::to_string(&wire).expect("VaultRecordJson has no non-serializable fields")
    }

    /// Parses a record previously produced by [`VaultRecord::to_json`].
    /// Any malformed JSON, base64, or field length maps to
    /// [`VaultError::Corrupt`] — never a panic on untrusted disk data.
    pub fn from_json(s: &str) -> Result<VaultRecord, VaultError> {
        let wire: VaultRecordJson = serde_json::from_str(s).map_err(|_| VaultError::Corrupt)?;
        let dek_wrapped_pass = STANDARD
            .decode(wire.dek_wrapped_pass)
            .map_err(|_| VaultError::Corrupt)?;
        let recovery_salt: [u8; 16] = STANDARD
            .decode(wire.recovery_salt)
            .map_err(|_| VaultError::Corrupt)?
            .try_into()
            .map_err(|_| VaultError::Corrupt)?;
        let dek_wrapped_recovery = STANDARD
            .decode(wire.dek_wrapped_recovery)
            .map_err(|_| VaultError::Corrupt)?;
        let dek_check = match wire.dek_check {
            None => None,
            Some(b64) => Some(STANDARD.decode(b64).map_err(|_| VaultError::Corrupt)?),
        };
        Ok(VaultRecord {
            kdf_params: wire.kdf_params,
            dek_wrapped_pass,
            recovery_salt,
            dek_wrapped_recovery,
            dek_check,
        })
    }
}

/// Builds the [`KdfParams`] used to derive a KEK from the recovery key:
/// same cost parameters as the default, but salted with `recovery_salt` so
/// the recovery-key KEK differs from the passphrase KEK.
fn recovery_kdf_params(recovery_salt: [u8; 16]) -> KdfParams {
    KdfParams {
        salt: recovery_salt,
        ..KdfParams::new_default()
    }
}

/// A record standing in for a server-side **recovery** entry: it carries only
/// the recovery-key wrapping, so [`unlock_recovery`] (which reads
/// `recovery_salt` / `dek_wrapped_recovery` and derives its own KDF params
/// from the salt) works against it, while the passphrase half stays empty.
///
/// Used by `ops::unlock_entries_with_recovery`, where the workspace hands
/// back one recovery wrap per key generation rather than whole records.
/// `kdf_params` is the recovery-side derivation and is deliberately never
/// used to unwrap `dek_wrapped_pass` — that field is empty here, so
/// [`unlock_passphrase`] on such a record always fails.
pub fn recovery_only_record(
    recovery_salt: [u8; 16],
    dek_wrapped_recovery: Vec<u8>,
    dek_check: Option<Vec<u8>>,
) -> VaultRecord {
    VaultRecord {
        kdf_params: recovery_kdf_params(recovery_salt),
        dek_wrapped_pass: Vec::new(),
        recovery_salt,
        dek_wrapped_recovery,
        dek_check,
    }
}

/// Creates a brand-new vault: generates a random DEK, wraps it under both
/// the given passphrase and a freshly generated recovery key, and returns
/// the resulting record alongside the one-time recovery key and the DEK
/// itself (so the caller can start using the vault immediately without a
/// redundant unlock).
#[allow(dead_code)]
pub fn setup(passphrase: &str) -> Result<(VaultRecord, RecoveryKey, Dek), VaultError> {
    let dek = Dek::random();

    let kdf_params = KdfParams::new_default();
    let dek_wrapped_pass = wrap_dek(&derive_kek(passphrase, &kdf_params)?, &dek);

    let recovery_key = RecoveryKey::generate();
    let mut recovery_salt = [0u8; 16];
    rand::rng().fill_bytes(&mut recovery_salt);
    let recovery_params = recovery_kdf_params(recovery_salt);
    let normalized_recovery = RecoveryKey::normalize(recovery_key.as_str());
    let dek_wrapped_recovery = wrap_dek(&derive_kek(&normalized_recovery, &recovery_params)?, &dek);

    let record = VaultRecord {
        kdf_params,
        dek_wrapped_pass,
        recovery_salt,
        dek_wrapped_recovery,
        dek_check: Some(make_dek_check(&dek)),
    };
    Ok((record, recovery_key, dek))
}

/// Unlocks the vault DEK using the passphrase.
#[allow(dead_code)]
pub fn unlock_passphrase(rec: &VaultRecord, passphrase: &str) -> Result<Dek, VaultError> {
    let kek = derive_kek(passphrase, &rec.kdf_params)?;
    unwrap_dek(&kek, &rec.dek_wrapped_pass)
}

/// Unlocks the vault DEK using the recovery key (accepts any formatting
/// the user typed — separators and case are normalized first).
#[allow(dead_code)]
pub fn unlock_recovery(rec: &VaultRecord, recovery_input: &str) -> Result<Dek, VaultError> {
    let normalized = RecoveryKey::normalize(recovery_input);
    let recovery_params = recovery_kdf_params(rec.recovery_salt);
    let kek = derive_kek(&normalized, &recovery_params)?;
    unwrap_dek(&kek, &rec.dek_wrapped_recovery)
}

/// Re-wraps the same DEK under a new passphrase (with fresh KDF params),
/// leaving the recovery-key wrapping untouched so the existing recovery key
/// keeps working after a passphrase change.
#[allow(dead_code)]
pub fn rewrap_passphrase(rec: &VaultRecord, dek: &Dek, new_passphrase: &str) -> VaultRecord {
    let kdf_params = KdfParams::new_default();
    let kek = derive_kek(new_passphrase, &kdf_params)
        .expect("KdfParams::new_default() always produces valid Argon2 parameters");
    let dek_wrapped_pass = wrap_dek(&kek, dek);
    VaultRecord {
        kdf_params,
        dek_wrapped_pass,
        recovery_salt: rec.recovery_salt,
        dek_wrapped_recovery: rec.dek_wrapped_recovery.clone(),
        // Always (re)establish the check: a legacy record without one is
        // upgraded here too, since the caller just proved it owns `dek`.
        dek_check: Some(make_dek_check(dek)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn setup_then_unlock_both_ways() {
        let (rec, rk, dek) = setup("hunter2").unwrap();
        assert_eq!(
            unlock_passphrase(&rec, "hunter2").unwrap().expose(),
            dek.expose()
        );
        assert_eq!(
            unlock_recovery(&rec, rk.as_str()).unwrap().expose(),
            dek.expose()
        );
        assert!(unlock_passphrase(&rec, "nope").is_err());
    }
    #[test]
    fn rewrap_changes_passphrase_keeps_dek() {
        let (rec, _rk, dek) = setup("old").unwrap();
        let rec2 = rewrap_passphrase(&rec, &dek, "new");
        assert!(unlock_passphrase(&rec2, "old").is_err());
        assert_eq!(
            unlock_passphrase(&rec2, "new").unwrap().expose(),
            dek.expose()
        );
    }
    #[test]
    fn dek_check_verifies_the_owning_dek_and_rejects_another() {
        let (rec, _rk, dek) = setup("pw").unwrap();
        assert!(rec.dek_check.is_some(), "setup writes the check");
        assert!(verify_dek(&rec, &dek).unwrap());
        let other = Dek::random();
        assert!(matches!(
            verify_dek(&rec, &other),
            Err(VaultError::WrongKey)
        ));
        // A rewrap keeps the same DEK, so the new record still verifies it.
        let rec2 = rewrap_passphrase(&rec, &dek, "pw2");
        assert!(verify_dek(&rec2, &dek).unwrap());
    }

    #[test]
    fn legacy_record_without_check_parses_and_reports_unverified() {
        // Exactly what a pre-upgrade record looks like on disk: no dek_check.
        let (rec, _rk, dek) = setup("pw").unwrap();
        let mut legacy = rec;
        legacy.dek_check = None;
        let json = legacy.to_json();
        assert!(!json.contains("dek_check"), "absent, not null: {json}");
        let parsed = VaultRecord::from_json(&json).unwrap();
        assert!(parsed.dek_check.is_none());
        assert!(!verify_dek(&parsed, &dek).unwrap());
        // Unlocking a legacy record still works (nothing to verify against).
        assert_eq!(
            unlock_passphrase(&parsed, "pw").unwrap().expose(),
            dek.expose()
        );
    }

    #[test]
    fn record_json_roundtrips() {
        let (rec, _, _) = setup("x").unwrap();
        let rec2 = VaultRecord::from_json(&rec.to_json()).unwrap();
        assert_eq!(rec.dek_wrapped_pass, rec2.dek_wrapped_pass);
    }
}
