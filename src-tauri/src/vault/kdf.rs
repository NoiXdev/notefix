use crate::vault::aead::{open, seal, Dek};
use crate::vault::VaultError;
use argon2::{Algorithm, Argon2, Params, Version};
use rand::Rng;
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

const WRAP_AAD: &[u8] = b"notefix-dek-v1";

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Clone)]
pub struct KdfParams {
    pub salt: [u8; 16],
    pub m_cost: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

/// Upper bounds for KDF parameters that arrive over the wire (the workspace
/// key cache, an invite wrap). Generous next to `new_default`'s 19 MiB / t=2 /
/// p=1 — a future server may legitimately raise the cost — but finite: a
/// server that sent `m_cost: u32::MAX` would otherwise have this device try to
/// allocate 4 TiB inside an unlock. Zero is rejected too; Argon2 refuses it,
/// and treating the entry as unopenable is the honest answer either way.
const MAX_M_COST: u32 = 1_048_576; // KiB, i.e. 1 GiB
const MAX_T_COST: u32 = 16;
const MAX_P_COST: u32 = 8;

#[allow(dead_code)]
impl KdfParams {
    pub fn new_default() -> Self {
        let mut salt = [0u8; 16];
        rand::rng().fill_bytes(&mut salt);
        KdfParams {
            salt,
            m_cost: 19_456,
            t_cost: 2,
            p_cost: 1,
        }
    }

    /// Whether these parameters are within the bounds above. Everything that
    /// parses SERVER-supplied parameters checks this before they can reach
    /// Argon2 — see `ops::MyEntry::try_from` and `ops::open_invite_wrap`.
    pub fn is_within_limits(&self) -> bool {
        (1..=MAX_M_COST).contains(&self.m_cost)
            && (1..=MAX_T_COST).contains(&self.t_cost)
            && (1..=MAX_P_COST).contains(&self.p_cost)
    }
}

#[allow(dead_code)]
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct Kek([u8; 32]);

#[allow(dead_code)]
pub fn derive_kek(passphrase: &str, p: &KdfParams) -> Result<Kek, VaultError> {
    let params = Params::new(p.m_cost, p.t_cost, p.p_cost, Some(32))
        .map_err(|e| VaultError::Crypto(e.to_string()))?;
    let a = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut out = [0u8; 32];
    a.hash_password_into(passphrase.as_bytes(), &p.salt, &mut out)
        .map_err(|e| VaultError::Crypto(e.to_string()))?;
    Ok(Kek(out))
}

#[allow(dead_code)]
pub fn wrap_dek(kek: &Kek, dek: &Dek) -> Vec<u8> {
    let kdek = Dek::from_bytes(kek.0);
    seal(&kdek, WRAP_AAD, dek.expose())
}

#[allow(dead_code)]
pub fn unwrap_dek(kek: &Kek, wrapped: &[u8]) -> Result<Dek, VaultError> {
    let kdek = Dek::from_bytes(kek.0);
    let bytes = open(&kdek, WRAP_AAD, wrapped)?;
    let arr: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| VaultError::Corrupt)?;
    Ok(Dek::from_bytes(arr))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::aead::Dek;
    #[test]
    fn wrap_unwrap_roundtrip() {
        let p = KdfParams::new_default();
        let kek = derive_kek("correct horse", &p).unwrap();
        let dek = Dek::random();
        let wrapped = wrap_dek(&kek, &dek);
        let out = unwrap_dek(&kek, &wrapped).unwrap();
        assert_eq!(out.expose(), dek.expose());
    }
    /// R4: parameters that arrive from a server must not be able to make an
    /// unlock allocate gigabytes or run for minutes.
    #[test]
    fn wire_parameters_outside_the_limits_are_rejected() {
        let base = KdfParams::new_default();
        assert!(base.is_within_limits());

        let with = |m: u32, t: u32, p: u32| KdfParams {
            salt: base.salt,
            m_cost: m,
            t_cost: t,
            p_cost: p,
        };
        // Upper bounds hold; one step past each one does not.
        assert!(with(MAX_M_COST, MAX_T_COST, MAX_P_COST).is_within_limits());
        assert!(!with(MAX_M_COST + 1, 2, 1).is_within_limits());
        assert!(!with(19_456, MAX_T_COST + 1, 1).is_within_limits());
        assert!(!with(19_456, 2, MAX_P_COST + 1).is_within_limits());
        assert!(!with(u32::MAX, 2, 1).is_within_limits());
        // Zero is not "cheap", it is invalid.
        assert!(!with(0, 2, 1).is_within_limits());
        assert!(!with(19_456, 0, 1).is_within_limits());
        assert!(!with(19_456, 2, 0).is_within_limits());
    }

    #[test]
    fn wrong_passphrase_fails() {
        let p = KdfParams::new_default();
        let dek = Dek::random();
        let wrapped = wrap_dek(&derive_kek("right", &p).unwrap(), &dek);
        let kek2 = derive_kek("wrong", &p).unwrap();
        assert!(unwrap_dek(&kek2, &wrapped).is_err());
    }
}
