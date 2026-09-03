//! In-memory runtime state for the protected-notes vault: holds a ring of
//! unlocked DEKs keyed by key generation, plus the last-activity timestamp
//! used for auto-lock. Never persisted — a process restart always starts
//! locked.
//!
//! A local (non-rotated) context's ring holds exactly generation 1. Once a
//! workspace rotates its key, unlocking hands over every generation still
//! needed to open existing ciphertext (`unlock` inserts/replaces one
//! generation at a time), while every NEW seal always uses the newest one
//! (`dek`/`newest_generation`).

use std::collections::BTreeMap;

use super::aead::Dek;

#[derive(Default)]
pub struct VaultState {
    /// generation → DEK. Local contexts hold exactly generation 1.
    ring: BTreeMap<u32, Dek>,
    // Written by `touch`; read by the auto-lock timer landing in a later
    // task, so it carries `#[allow(dead_code)]` until that caller lands.
    #[allow(dead_code)]
    last_active: Option<i64>,
}

impl VaultState {
    /// Store a freshly unlocked DEK for `generation`, replacing any previous
    /// DEK for that same generation. Other generations already in the ring
    /// are left untouched.
    pub fn unlock(&mut self, generation: u32, dek: Dek) {
        self.ring.insert(generation, dek);
    }

    /// Clear every generation, returning the vault to a locked state.
    pub fn lock(&mut self) {
        self.ring.clear();
    }

    /// True while any generation is unlocked.
    pub fn is_unlocked(&self) -> bool {
        !self.ring.is_empty()
    }

    /// The newest generation's DEK — the one every new seal uses. `None`
    /// while the vault is locked.
    pub fn dek(&self) -> Option<&Dek> {
        self.ring.iter().next_back().map(|(_, d)| d)
    }

    /// The newest generation currently unlocked, or `None` while locked.
    pub fn newest_generation(&self) -> Option<u32> {
        self.ring.keys().next_back().copied()
    }

    /// The DEK a note was sealed with. `None` (pre-generation notes, sealed
    /// before schema v15) is treated as generation 1. `None` is also
    /// returned when that generation simply isn't in the ring yet.
    pub fn dek_for(&self, generation: Option<u32>) -> Option<&Dek> {
        self.ring.get(&generation.unwrap_or(1))
    }

    /// Every generation currently unlocked, ascending — the creator's
    /// recovery follow-up walks it to find the DEKs it can still wrap
    /// (`commands::vault_recovery_followup`).
    pub fn generations(&self) -> Vec<u32> {
        self.ring.keys().copied().collect()
    }

    /// Record `now` (ms since epoch) as the last activity time, used by the
    /// auto-lock timer. Not yet called from app code — wired in by a later
    /// task — so it carries `#[allow(dead_code)]` per the Task 1/2 precedent
    /// in `vault/`.
    #[allow(dead_code)]
    pub fn touch(&mut self, now: i64) {
        self.last_active = Some(now);
    }

    /// Every generation with a clone of its DEK, ascending — the keychain
    /// snapshot for biometric unlock. Clones live only as long as the caller
    /// keeps them (`Dek` zeroizes on drop).
    pub fn snapshot(&self) -> Vec<(u32, Dek)> {
        self.ring.iter().map(|(g, d)| (*g, d.clone())).collect()
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
        s.unlock(1, Dek::random());
        assert!(s.is_unlocked());
        s.lock();
        assert!(!s.is_unlocked() && s.dek().is_none());
    }

    #[test]
    fn ring_seals_with_newest_and_opens_by_generation() {
        let mut s = VaultState::default();
        assert!(!s.is_unlocked() && s.dek_for(None).is_none());
        let d1 = Dek::random();
        let d2 = Dek::random();
        s.unlock(1, d1.clone());
        s.unlock(2, d2.clone());
        assert_eq!(s.newest_generation(), Some(2));
        assert_eq!(s.dek().unwrap().expose(), d2.expose());
        assert_eq!(s.dek_for(Some(1)).unwrap().expose(), d1.expose());
        assert_eq!(
            s.dek_for(None).unwrap().expose(),
            d1.expose(),
            "legacy notes = generation 1"
        );
        assert!(s.dek_for(Some(3)).is_none());
        assert_eq!(s.generations(), vec![1, 2]);
        s.lock();
        assert!(!s.is_unlocked());
    }

    #[test]
    fn snapshot_lists_every_generation_ascending_with_its_key() {
        let mut s = VaultState::default();
        let (d1, d3) = (Dek::random(), Dek::random());
        s.unlock(3, d3.clone());
        s.unlock(1, d1.clone());
        let snap = s.snapshot();
        assert_eq!(snap.iter().map(|(g, _)| *g).collect::<Vec<_>>(), vec![1, 3]);
        assert_eq!(snap[0].1.expose(), d1.expose());
        assert_eq!(snap[1].1.expose(), d3.expose());
        assert!(VaultState::default().snapshot().is_empty());
    }
}
