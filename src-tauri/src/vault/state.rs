//! In-memory runtime state for the protected-notes vault: holds the unlocked
//! DEK (if any) and the last-activity timestamp used for auto-lock. Never
//! persisted — a process restart always starts locked.

use super::aead::Dek;

#[derive(Default)]
pub struct VaultState {
    dek: Option<Dek>,
    // Written by `touch`; read by the auto-lock timer landing in a later
    // task, so it carries `#[allow(dead_code)]` until that caller lands.
    #[allow(dead_code)]
    last_active: Option<i64>,
}

impl VaultState {
    /// Store the freshly unlocked DEK, replacing any previous one.
    pub fn unlock(&mut self, dek: Dek) {
        self.dek = Some(dek);
    }

    /// Clear the DEK, returning the vault to a locked state.
    pub fn lock(&mut self) {
        self.dek = None;
    }

    pub fn is_unlocked(&self) -> bool {
        self.dek.is_some()
    }

    /// The unlocked DEK, or `None` while the vault is locked. Used by Task 6's
    /// encrypt/decrypt commands to seal/open protected note content.
    pub fn dek(&self) -> Option<&Dek> {
        self.dek.as_ref()
    }

    /// Record `now` (ms since epoch) as the last activity time, used by the
    /// auto-lock timer. Not yet called from app code — wired in by a later
    /// task — so it carries `#[allow(dead_code)]` per the Task 1/2 precedent
    /// in `vault/`.
    #[allow(dead_code)]
    pub fn touch(&mut self, now: i64) {
        self.last_active = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::aead::Dek;

    #[test]
    fn lock_clears_dek() {
        let mut s = VaultState::default();
        assert!(!s.is_unlocked());
        s.unlock(Dek::random());
        assert!(s.is_unlocked());
        s.lock();
        assert!(!s.is_unlocked() && s.dek().is_none());
    }
}
