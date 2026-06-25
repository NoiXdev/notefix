// src-tauri/src/profiles.rs
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContextEntry {
    pub id: String,
    pub label: String,        // "" = i18n default label, frontend übersetzt
    pub kind: String,         // "local" | "server"
    pub path: String,         // absoluter DB-Pfad (lokaler Cache bei "server")
    #[serde(default)]
    pub server_url: String,   // nur bei kind == "server"
    #[serde(default)]
    pub workspace_id: String, // nur bei kind == "server"; "" = noch nicht gebunden
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
            contexts: vec![ContextEntry { id, label: String::new(), kind: "local".into(), path: db_path.into(), server_url: String::new(), workspace_id: String::new() }],
        }
    }

    pub fn add(&mut self, id: String, label: String, path: String) -> ContextEntry {
        let e = ContextEntry { id, label, kind: "local".into(), path, server_url: String::new(), workspace_id: String::new() };
        self.contexts.push(e.clone());
        e
    }

    /// Add a server-backed context (local cache DB at `path`, tokens in keychain).
    pub fn add_server(&mut self, id: String, label: String, path: String, server_url: String) -> ContextEntry {
        let e = ContextEntry { id, label, kind: "server".into(), path, server_url, workspace_id: String::new() };
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

    pub fn bind_workspace(&mut self, id: &str, workspace_id: String) -> Result<(), String> {
        let c = self.contexts.iter_mut().find(|c| c.id == id).ok_or("unknown context")?;
        c.workspace_id = workspace_id;
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
    match std::fs::read_to_string(path).ok().and_then(|j| serde_json::from_str::<Registry>(&j).ok()) {
        Some(r) if r.active().is_some() => r,
        _ => Registry::default_for(default_db),
    }
}

pub fn save(path: &Path, reg: &Registry) -> std::io::Result<()> {
    if let Some(p) = path.parent() { std::fs::create_dir_all(p)?; }
    std::fs::write(path, serde_json::to_string_pretty(reg).unwrap())
}

/// True if `path` is a legacy flat context DB sitting directly in the contexts
/// dir (e.g. `<contexts>/<id>.db`). Those predate the per-context subdirectory
/// layout (`<contexts>/<id>/notefix.db`) and must be relocated so each context
/// gets its own isolated images folder. The default context (under app_data,
/// not the contexts dir) and already-migrated contexts are not matched.
pub fn is_flat_context_path(path: &Path, contexts_dir: &Path) -> bool {
    path.parent() == Some(contexts_dir) && path.extension().map(|e| e == "db").unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, path: &str) -> ContextEntry {
        ContextEntry { id: id.into(), label: String::new(), kind: "local".into(), path: path.into(), server_url: String::new(), workspace_id: String::new() }
    }

    #[test]
    fn default_registry_points_at_given_db() {
        let r = Registry::default_for("/data/notefix.db");
        assert_eq!(r.contexts.len(), 1);
        assert_eq!(r.active().unwrap().path, "/data/notefix.db");
        assert_eq!(r.active().unwrap().kind, "local");
    }

    #[test]
    fn flat_context_path_detection() {
        let cdir = Path::new("/data/contexts");
        assert!(is_flat_context_path(Path::new("/data/contexts/abc.db"), cdir));
        // already migrated (in its own subfolder)
        assert!(!is_flat_context_path(Path::new("/data/contexts/abc/notefix.db"), cdir));
        // default context lives next to app_data, not in the contexts dir
        assert!(!is_flat_context_path(Path::new("/data/notefix.db"), cdir));
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
    fn add_server_marks_kind_and_url() {
        let mut r = Registry::default_for("/data/notefix.db");
        let e = r.add_server("s".into(), "notes.example".into(), "/data/contexts/s/notefix.db".into(), "https://notes.example".into());
        assert_eq!(e.kind, "server");
        assert_eq!(e.server_url, "https://notes.example");
        assert_eq!(r.contexts.len(), 2);
        r.set_active("s").unwrap();
        assert_eq!(r.active().unwrap().kind, "server");
    }

    #[test]
    fn bind_workspace_sets_id() {
        let mut r = Registry::default_for("/d.db");
        let e = r.add_server("s".into(), "srv".into(), "/s.db".into(), "https://s".into());
        r.bind_workspace(&e.id, "ws-1".into()).unwrap();
        assert_eq!(r.contexts.iter().find(|c| c.id == e.id).unwrap().workspace_id, "ws-1");
    }

    #[test]
    fn rename_changes_label() {
        let mut r = Registry::default_for("/data/notefix.db");
        let id = r.active_id.clone();
        r.rename(&id, "Arbeit".into()).unwrap();
        assert_eq!(r.active().unwrap().label, "Arbeit");
    }
}
