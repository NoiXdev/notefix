//! One-time recovery key: a human-writable secret that can unwrap the vault
//! DEK if the passphrase is lost. Not yet called from app code — wired in by
//! a later task — so items carry `#[allow(dead_code)]` per the Task 1/2
//! precedent in this module tree.

use rand::Rng;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Crockford base32 alphabet: no `I`, `L`, `O`, `U`, to avoid visual
/// ambiguity when a user transcribes the key by hand.
const CROCKFORD_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Number of random bytes backing a recovery key (160 bits).
const RECOVERY_KEY_BYTES: usize = 20;

/// Size of each dash-separated group in the formatted recovery key.
const GROUP_LEN: usize = 5;

/// A one-time **invite code**: the secret an owner reads out to an invited
/// member so their client can open the vault key wrapped onto the invitation.
/// Same construction as a recovery key — the same amount of entropy, the same
/// unambiguous alphabet, the same "type it however you like" normalization —
/// so it is a deliberate alias rather than a second implementation.
pub type InviteCode = RecoveryKey;

/// A one-time recovery key, formatted for display as dash-separated groups
/// of Crockford base32 characters (e.g. `ABCDE-FGHJK-...`).
///
/// Holds key material, so it zeroizes on drop like [`crate::vault::aead::Dek`].
#[allow(dead_code)]
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct RecoveryKey(String);

#[allow(dead_code)]
impl RecoveryKey {
    /// Generates a fresh recovery key from 20 bytes of randomness.
    pub fn generate() -> Self {
        let mut bytes = [0u8; RECOVERY_KEY_BYTES];
        rand::rng().fill_bytes(&mut bytes);
        RecoveryKey(group(&encode_crockford_base32(&bytes)))
    }

    /// Strips separators/whitespace and uppercases input so a recovery key
    /// can be derived the same way regardless of how the user typed it.
    pub fn normalize(input: &str) -> String {
        input
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-')
            .flat_map(char::to_uppercase)
            .collect()
    }

    /// The formatted recovery key, e.g. `ABCDE-FGHJK-MNPQR-STVWX-YZ012-3456`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Encodes `data` as Crockford base32 (no padding).
fn encode_crockford_base32(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(5) * 8);
    let mut bits: u32 = 0;
    let mut bit_count: u32 = 0;
    for &byte in data {
        bits = (bits << 8) | u32::from(byte);
        bit_count += 8;
        while bit_count >= 5 {
            bit_count -= 5;
            let idx = (bits >> bit_count) & 0x1F;
            out.push(CROCKFORD_ALPHABET[idx as usize] as char);
        }
    }
    if bit_count > 0 {
        let idx = (bits << (5 - bit_count)) & 0x1F;
        out.push(CROCKFORD_ALPHABET[idx as usize] as char);
    }
    out
}

/// Joins `s` into dash-separated groups of [`GROUP_LEN`] characters.
fn group(s: &str) -> String {
    s.as_bytes()
        .chunks(GROUP_LEN)
        .map(|chunk| std::str::from_utf8(chunk).expect("ASCII input"))
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_is_grouped_crockford() {
        let rk = RecoveryKey::generate();
        let s = rk.as_str();
        assert!(s.split('-').all(|g| !g.is_empty() && g.len() <= GROUP_LEN));
        let stripped: String = s.chars().filter(|c| *c != '-').collect();
        assert!(stripped
            .chars()
            .all(|c| CROCKFORD_ALPHABET.contains(&(c as u8))));
    }

    #[test]
    fn normalize_strips_separators_and_uppercases() {
        assert_eq!(
            RecoveryKey::normalize(" abcde-fghjk \n"),
            "ABCDEFGHJK".to_string()
        );
    }

    #[test]
    fn normalize_of_generated_key_matches_stripped_form() {
        let rk = RecoveryKey::generate();
        let expected: String = rk.as_str().chars().filter(|c| *c != '-').collect();
        assert_eq!(RecoveryKey::normalize(rk.as_str()), expected);
    }
}
