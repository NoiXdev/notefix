// src-tauri/src/profiles.rs
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContextEntry {
    pub id: String,
    pub label: String,        // "" = i18n default label, frontend übersetzt
    pub kind: String,         // "local" (Vorbereitung für "server")
    pub path: String,         // absoluter DB-Pfad
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Registry {
    pub version: u32,
    pub active_id: String,
    pub contexts: Vec<ContextEntry>,
}

impl Registry {
    pub fn active(&self) -> Option<&ContextEntry> {
        self.contexts.iter().find(|c| c.id == self.active_id)
    }

    pub fn default_for(db_path: &str) -> Self {
        let id = new_id();
        Registry {
            version: 1,
            active_id: id.clone(),
            contexts: vec![ContextEntry { id, label: String::new(), kind: "local".into(), path: db_path.into() }],
        }
    }

    pub fn add(&mut self, id: String, label: String, path: String) -> ContextEntry {
        let e = ContextEntry { id, label, kind: "local".into(), path };
        self.contexts.push(e.clone());
        e
    }

    pub fn set_active(&mut self, id: &str) -> Result<(), String> {
        if !self.contexts.iter().any(|c| c.id == id) { return Err("unknown context".into()); }
        self.active_id = id.into();
        Ok(())
    }

    pub fn rename(&mut self, id: &str, label: String) -> Result<(), String> {
        let c = self.contexts.iter_mut().find(|c| c.id == id).ok_or("unknown context")?;
        c.label = label;
        Ok(())
    }

    pub fn remove(&mut self, id: &str) -> Result<ContextEntry, String> {
        if id == self.active_id { return Err("cannot remove active context".into()); }
        if self.contexts.len() <= 1 { return Err("cannot remove last context".into()); }
        let pos = self.contexts.iter().position(|c| c.id == id).ok_or("unknown context")?;
        Ok(self.contexts.remove(pos))
    }
}

fn new_id() -> String { uuid::Uuid::new_v4().to_string() }

// Persistence (not unit-tested — needs a real file):
pub fn load(path: &Path, default_db: &str) -> Registry {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|j| serde_json::from_str(&j).ok())
        .unwrap_or_else(|| Registry::default_for(default_db))
}

pub fn save(path: &Path, reg: &Registry) -> std::io::Result<()> {
    if let Some(p) = path.parent() { std::fs::create_dir_all(p)?; }
    std::fs::write(path, serde_json::to_string_pretty(reg).unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, path: &str) -> ContextEntry {
        ContextEntry { id: id.into(), label: String::new(), kind: "local".into(), path: path.into() }
    }

    #[test]
    fn default_registry_points_at_given_db() {
        let r = Registry::default_for("/data/notefix.db");
        assert_eq!(r.contexts.len(), 1);
        assert_eq!(r.active().unwrap().path, "/data/notefix.db");
        assert_eq!(r.active().unwrap().kind, "local");
    }

    #[test]
    fn roundtrips_through_json() {
        let r = Registry { version: 1, active_id: "a".into(), contexts: vec![entry("a", "/x.db")] };
        let json = serde_json::to_string(&r).unwrap();
        let back: Registry = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn add_appends_and_switches() {
        let mut r = Registry::default_for("/data/notefix.db");
        let e = r.add("b".into(), "My DB".into(), "/data/contexts/b.db".into());
        assert_eq!(e.id, "b");
        assert_eq!(r.contexts.len(), 2);
        r.set_active("b").unwrap();
        assert_eq!(r.active_id, "b");
    }

    #[test]
    fn cannot_remove_active_or_last() {
        let mut r = Registry::default_for("/data/notefix.db");
        let only = r.active_id.clone();
        assert!(r.remove(&only).is_err()); // last + active
        r.add("b".into(), String::new(), "/b.db".into());
        assert!(r.remove(&r.active_id.clone()).is_err()); // active
        assert!(r.remove("b").is_ok()); // non-active ok
    }

    #[test]
    fn rename_changes_label() {
        let mut r = Registry::default_for("/data/notefix.db");
        let id = r.active_id.clone();
        r.rename(&id, "Arbeit".into()).unwrap();
        assert_eq!(r.active().unwrap().label, "Arbeit");
    }
}
