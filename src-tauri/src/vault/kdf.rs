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
    #[test]
    fn wrong_passphrase_fails() {
        let p = KdfParams::new_default();
        let dek = Dek::random();
        let wrapped = wrap_dek(&derive_kek("right", &p).unwrap(), &dek);
        let kek2 = derive_kek("wrong", &p).unwrap();
        assert!(unwrap_dek(&kek2, &wrapped).is_err());
    }
}
