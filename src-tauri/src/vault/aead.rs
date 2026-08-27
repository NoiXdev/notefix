use crate::vault::VaultError;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand::RngCore;
use zeroize::{Zeroize, ZeroizeOnDrop};

// The items below are not yet called from app code — they're consumed
// starting with Task 2 (KDF/DEK wrapping) and wired end-to-end by Task 5/6.
// Test coverage exercises this module already; remove these allows once a
// non-test caller lands.
#[allow(dead_code)]
const NONCE_LEN: usize = 24;

#[allow(dead_code)]
#[derive(Zeroize, ZeroizeOnDrop, Clone)]
pub struct Dek([u8; 32]);

#[allow(dead_code)]
impl Dek {
    pub fn random() -> Self {
        let mut b = [0u8; 32];
        rand::rng().fill_bytes(&mut b);
        Dek(b)
    }
    pub fn from_bytes(b: [u8; 32]) -> Self {
        Dek(b)
    }
    pub fn expose(&self) -> &[u8; 32] {
        &self.0
    }
}

#[allow(dead_code)]
pub fn seal(dek: &Dek, aad: &[u8], plaintext: &[u8]) -> Vec<u8> {
    let cipher = XChaCha20Poly1305::new(dek.expose().into());
    let mut nonce = [0u8; NONCE_LEN];
    rand::rng().fill_bytes(&mut nonce);
    let ct = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .expect("AEAD encryption never fails with valid key/nonce");
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    out
}

#[allow(dead_code)]
pub fn open(dek: &Dek, aad: &[u8], blob: &[u8]) -> Result<Vec<u8>, VaultError> {
    if blob.len() < NONCE_LEN + 16 {
        return Err(VaultError::Corrupt);
    }
    let (nonce, ct) = blob.split_at(NONCE_LEN);
    let cipher = XChaCha20Poly1305::new(dek.expose().into());
    cipher
        .decrypt(XNonce::from_slice(nonce), Payload { msg: ct, aad })
        .map_err(|_| VaultError::WrongKey)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seal_open_roundtrip() {
        let dek = Dek::random();
        let blob = seal(&dek, b"note-1", b"hello");
        assert_eq!(open(&dek, b"note-1", &blob).unwrap(), b"hello");
    }
    #[test]
    fn open_rejects_wrong_aad() {
        let dek = Dek::random();
        let blob = seal(&dek, b"note-1", b"hello");
        assert!(open(&dek, b"note-2", &blob).is_err());
    }
    #[test]
    fn open_rejects_tampered_ciphertext() {
        let dek = Dek::random();
        let mut blob = seal(&dek, b"n", b"hello");
        let last = blob.len() - 1;
        blob[last] ^= 0x01;
        assert!(open(&dek, b"n", &blob).is_err());
    }
    #[test]
    fn open_rejects_wrong_key() {
        let blob = seal(&Dek::random(), b"n", b"hello");
        assert!(open(&Dek::random(), b"n", &blob).is_err());
    }
}
