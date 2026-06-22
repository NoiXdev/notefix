use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub content: String,
    pub updated_at: i64,
    // `default` lets legacy dginx-notes JSON (no `pinned` field) deserialize to false.
    #[serde(default)]
    pub pinned: bool,
}

pub struct Store {
    pub conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        Ok(Self { conn: Connection::open(path)? })
    }

    #[cfg(test)]
    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Ok(Self { conn: Connection::open_in_memory()? })
    }

    /// Pinned first, then newest first.
    pub fn load_notes(&self) -> rusqlite::Result<Vec<Note>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content, updated_at, pinned FROM notes ORDER BY pinned DESC, updated_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Note {
                id: r.get(0)?,
                content: r.get(1)?,
                updated_at: r.get(2)?,
                pinned: r.get(3)?,
            })
        })?;
        rows.collect()
    }

    /// Insert or update by id. On conflict, ONLY content + updated_at change —
    /// `pinned` is preserved (a content edit must never clear the pin).
    pub fn save_note(&self, note: &Note) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO notes (id, content, updated_at, pinned) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(id) DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at",
            (&note.id, &note.content, note.updated_at, note.pinned),
        )?;
        Ok(())
    }

    pub fn delete_note(&self, id: &str) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM notes WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Set the pin flag only. Does NOT touch `updated_at` (pinning is not a content edit).
    pub fn set_pinned(&self, id: &str, pinned: bool) -> rusqlite::Result<()> {
        self.conn.execute("UPDATE notes SET pinned = ?2 WHERE id = ?1", (id, pinned))?;
        Ok(())
    }

    /// The `limit` most-recently-updated notes (newest first), ignoring pin order.
    pub fn recent_notes(&self, limit: i64) -> rusqlite::Result<Vec<Note>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content, updated_at, pinned FROM notes ORDER BY updated_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit], |r| {
            Ok(Note { id: r.get(0)?, content: r.get(1)?, updated_at: r.get(2)?, pinned: r.get(3)? })
        })?;
        rows.collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrate;

    fn store() -> Store {
        let s = Store::open_in_memory().unwrap();
        migrate::run_migrations(&s.conn).unwrap();
        s
    }

    fn note(id: &str, content: &str, updated_at: i64) -> Note {
        Note { id: id.into(), content: content.into(), updated_at, pinned: false }
    }

    #[test]
    fn loads_empty_when_no_notes() {
        assert_eq!(store().load_notes().unwrap(), vec![]);
    }

    #[test]
    fn saves_and_loads_a_note() {
        let s = store();
        s.save_note(&note("a", "<p>hi</p>", 1000)).unwrap();
        assert_eq!(s.load_notes().unwrap(), vec![note("a", "<p>hi</p>", 1000)]);
    }

    #[test]
    fn new_note_defaults_unpinned() {
        let s = store();
        s.save_note(&note("a", "<p>x</p>", 1000)).unwrap();
        assert!(!s.load_notes().unwrap()[0].pinned);
    }

    #[test]
    fn set_pinned_toggles_flag() {
        let s = store();
        s.save_note(&note("a", "<p>x</p>", 1000)).unwrap();
        s.set_pinned("a", true).unwrap();
        assert!(s.load_notes().unwrap()[0].pinned);
    }

    #[test]
    fn set_pinned_does_not_change_updated_at() {
        let s = store();
        s.save_note(&note("a", "<p>x</p>", 1000)).unwrap();
        s.set_pinned("a", true).unwrap();
        assert_eq!(s.load_notes().unwrap()[0].updated_at, 1000);
    }

    #[test]
    fn content_update_preserves_pinned() {
        let s = store();
        s.save_note(&note("a", "<p>v1</p>", 1000)).unwrap();
        s.set_pinned("a", true).unwrap();
        s.save_note(&note("a", "<p>v2</p>", 2000)).unwrap();
        let loaded = &s.load_notes().unwrap()[0];
        assert!(loaded.pinned);
        assert_eq!(loaded.content, "<p>v2</p>");
        assert_eq!(loaded.updated_at, 2000);
    }

    #[test]
    fn pinned_sorts_to_top_regardless_of_date() {
        let s = store();
        s.save_note(&note("old", "<p>old</p>", 1000)).unwrap();
        s.save_note(&note("new", "<p>new</p>", 2000)).unwrap();
        s.set_pinned("old", true).unwrap();
        let ids: Vec<String> = s.load_notes().unwrap().into_iter().map(|n| n.id).collect();
        assert_eq!(ids, vec!["old", "new"]);
    }

    #[test]
    fn recent_notes_orders_by_updated_at_ignoring_pin() {
        let s = store();
        s.save_note(&note("a", "<p>a</p>", 1000)).unwrap();
        s.save_note(&note("b", "<p>b</p>", 3000)).unwrap();
        s.save_note(&note("c", "<p>c</p>", 2000)).unwrap();
        s.set_pinned("a", true).unwrap(); // pin oldest — must NOT jump the queue here
        let ids: Vec<String> = s.recent_notes(2).unwrap().into_iter().map(|n| n.id).collect();
        assert_eq!(ids, vec!["b", "c"]);
    }
}
