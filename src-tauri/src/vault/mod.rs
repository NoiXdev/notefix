pub mod aead;

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
