use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub content: String,
    pub updated_at: i64,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub due_at: Option<i64>,
    #[serde(default)]
    pub folder_id: Option<String>,
    #[serde(default)]
    pub position: i64,
    #[serde(default)]
    pub deleted_at: Option<i64>,
}

pub struct Store {
    pub conn: Connection,
}

const COLS: &str = "id, content, updated_at, pinned, archived, color, due_at, folder_id, position, deleted_at";

fn row_to_note(r: &rusqlite::Row) -> rusqlite::Result<Note> {
    Ok(Note {
        id: r.get(0)?, content: r.get(1)?, updated_at: r.get(2)?,
        pinned: r.get(3)?, archived: r.get(4)?, color: r.get(5)?, due_at: r.get(6)?,
        folder_id: r.get(7)?, position: r.get(8)?, deleted_at: r.get(9)?,
    })
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        Ok(Self { conn: Connection::open(path)? })
    }

    #[cfg(test)]
    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Ok(Self { conn: Connection::open_in_memory()? })
    }

    pub fn load_notes(&self) -> rusqlite::Result<Vec<Note>> {
        let sql = format!("SELECT {COLS} FROM notes WHERE deleted_at IS NULL ORDER BY pinned DESC, position ASC");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], row_to_note)?;
        rows.collect()
    }

    /// Alle Notizen inkl. Papierkorb (für Migration/GC-Referenzscan).
    pub fn load_all_notes(&self) -> rusqlite::Result<Vec<Note>> {
        let sql = format!("SELECT {COLS} FROM notes");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], row_to_note)?;
        rows.collect()
    }

    /// Nur den Content setzen — ohne `updated_at`-Bump und ohne Revision.
    pub fn set_content_silent(&self, id: &str, content: &str) -> rusqlite::Result<()> {
        self.conn.execute("UPDATE notes SET content = ?2 WHERE id = ?1", (id, content))?;
        Ok(())
    }

    pub fn save_note(&self, note: &Note) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO notes (id, content, updated_at, pinned, archived, color, due_at, folder_id, position) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at",
            (&note.id, &note.content, note.updated_at, note.pinned, note.archived, &note.color, note.due_at, &note.folder_id, note.position),
        )?;
        Ok(())
    }

    pub fn delete_note(&self, id: &str) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM note_revisions WHERE note_id = ?1", [id])?;
        self.conn.execute("DELETE FROM notes WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn set_pinned(&self, id: &str, pinned: bool) -> rusqlite::Result<()> {
        self.conn.execute("UPDATE notes SET pinned = ?2 WHERE id = ?1", (id, pinned))?;
        Ok(())
    }

    pub fn set_archived(&self, id: &str, archived: bool) -> rusqlite::Result<()> {
        self.conn.execute("UPDATE notes SET archived = ?2 WHERE id = ?1", (id, archived))?;
        Ok(())
    }

    pub fn set_color(&self, id: &str, color: &str) -> rusqlite::Result<()> {
        self.conn.execute("UPDATE notes SET color = ?2 WHERE id = ?1", (id, color))?;
        Ok(())
    }

    /// Set or clear the due date. Does NOT touch `updated_at`.
    pub fn set_due(&self, id: &str, due_at: Option<i64>) -> rusqlite::Result<()> {
        self.conn.execute("UPDATE notes SET due_at = ?2 WHERE id = ?1", (id, due_at))?;
        Ok(())
    }

    /// Move a note to a folder (None = root). Does NOT touch `updated_at`.
    pub fn set_folder(&self, id: &str, folder_id: Option<&str>) -> rusqlite::Result<()> {
        self.conn.execute("UPDATE notes SET folder_id = ?2 WHERE id = ?1", (id, folder_id))?;
        Ok(())
    }

    /// Set folder + position for each id in the given order.
    pub fn reorder_notes(&self, folder_id: Option<&str>, ids: &[String]) -> rusqlite::Result<()> {
        for (i, id) in ids.iter().enumerate() {
            self.conn.execute("UPDATE notes SET folder_id = ?2, position = ?3 WHERE id = ?1", (id, folder_id, i as i64))?;
        }
        Ok(())
    }

    /// The `limit` most-recently-updated NON-archived notes (newest first).
    pub fn recent_notes(&self, limit: i64) -> rusqlite::Result<Vec<Note>> {
        let sql = format!("SELECT {COLS} FROM notes WHERE archived = 0 AND deleted_at IS NULL ORDER BY updated_at DESC LIMIT ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([limit], row_to_note)?;
        rows.collect()
    }

    pub fn trash_note(&self, id: &str, ts: i64) -> rusqlite::Result<()> {
        self.conn.execute("UPDATE notes SET deleted_at = ?2 WHERE id = ?1", (id, ts))?;
        Ok(())
    }

    pub fn restore_note(&self, id: &str) -> rusqlite::Result<()> {
        self.conn.execute("UPDATE notes SET deleted_at = NULL WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn load_trashed(&self) -> rusqlite::Result<Vec<Note>> {
        let sql = format!("SELECT {COLS} FROM notes WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], row_to_note)?;
        rows.collect()
    }

    pub fn purge_trashed(&self, before: Option<i64>) -> rusqlite::Result<()> {
        match before {
            Some(t) => {
                self.conn.execute("DELETE FROM note_revisions WHERE note_id IN (SELECT id FROM notes WHERE deleted_at IS NOT NULL AND deleted_at < ?1)", [t])?;
                self.conn.execute("DELETE FROM notes WHERE deleted_at IS NOT NULL AND deleted_at < ?1", [t])?;
            }
            None => {
                self.conn.execute("DELETE FROM note_revisions WHERE note_id IN (SELECT id FROM notes WHERE deleted_at IS NOT NULL)", [])?;
                self.conn.execute("DELETE FROM notes WHERE deleted_at IS NOT NULL", [])?;
            }
        }
        Ok(())
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
        Note { id: id.into(), content: content.into(), updated_at, pinned: false, archived: false, color: String::new(), due_at: None, folder_id: None, position: 0, deleted_at: None }
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
    fn new_note_has_no_due_date() {
        let s = store();
        s.save_note(&note("a", "<p>x</p>", 1000)).unwrap();
        assert_eq!(s.load_notes().unwrap()[0].due_at, None);
    }

    #[test]
    fn set_due_sets_and_clears_without_touching_updated_at() {
        let s = store();
        s.save_note(&note("a", "<p>x</p>", 1000)).unwrap();
        s.set_due("a", Some(5000)).unwrap();
        let n = &s.load_notes().unwrap()[0];
        assert_eq!(n.due_at, Some(5000));
        assert_eq!(n.updated_at, 1000);
        s.set_due("a", None).unwrap();
        assert_eq!(s.load_notes().unwrap()[0].due_at, None);
    }

    #[test]
    fn content_update_preserves_due_date() {
        let s = store();
        s.save_note(&note("a", "<p>v1</p>", 1000)).unwrap();
        s.set_due("a", Some(7000)).unwrap();
        s.save_note(&note("a", "<p>v2</p>", 2000)).unwrap();
        assert_eq!(s.load_notes().unwrap()[0].due_at, Some(7000));
    }

    #[test]
    fn set_pinned_archived_color_still_work() {
        let s = store();
        s.save_note(&note("a", "<p>x</p>", 1000)).unwrap();
        s.set_pinned("a", true).unwrap();
        s.set_archived("a", true).unwrap();
        s.set_color("a", "#ef4444").unwrap();
        let n = &s.load_notes().unwrap()[0];
        assert!(n.pinned && n.archived);
        assert_eq!(n.color, "#ef4444");
    }

    #[test]
    fn set_folder_moves_note_without_touching_updated_at() {
        let s = store();
        s.save_note(&note("a", "<p>x</p>", 1000)).unwrap();
        s.set_folder("a", Some("f1")).unwrap();
        let n = &s.load_notes().unwrap()[0];
        assert_eq!(n.folder_id.as_deref(), Some("f1"));
        assert_eq!(n.updated_at, 1000);
        s.set_folder("a", None).unwrap();
        assert_eq!(s.load_notes().unwrap()[0].folder_id, None);
    }

    #[test]
    fn reorder_notes_sets_folder_and_position() {
        let s = store();
        s.save_note(&note("a", "<p>a</p>", 1)).unwrap();
        s.save_note(&note("b", "<p>b</p>", 2)).unwrap();
        s.reorder_notes(Some("f1"), &["b".to_string(), "a".to_string()]).unwrap();
        let loaded = s.load_notes().unwrap();
        // both now in f1, ordered b(pos0) then a(pos1)
        assert_eq!(loaded.iter().map(|n| n.id.clone()).collect::<Vec<_>>(), vec!["b", "a"]);
        assert!(loaded.iter().all(|n| n.folder_id.as_deref() == Some("f1")));
        assert_eq!(loaded[0].position, 0);
        assert_eq!(loaded[1].position, 1);
    }

    #[test]
    fn recent_notes_excludes_archived() {
        let s = store();
        s.save_note(&note("a", "<p>a</p>", 1000)).unwrap();
        s.save_note(&note("b", "<p>b</p>", 2000)).unwrap();
        s.set_archived("b", true).unwrap();
        let ids: Vec<String> = s.recent_notes(5).unwrap().into_iter().map(|n| n.id).collect();
        assert_eq!(ids, vec!["a"]);
    }

    #[test]
    fn trash_hides_from_load_and_shows_in_trashed() {
        let s = store();
        s.save_note(&note("a", "<p>a</p>", 1)).unwrap();
        s.trash_note("a", 1000).unwrap();
        assert!(s.load_notes().unwrap().is_empty());
        let t = s.load_trashed().unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].deleted_at, Some(1000));
        s.restore_note("a").unwrap();
        assert_eq!(s.load_notes().unwrap().len(), 1);
        assert!(s.load_trashed().unwrap().is_empty());
    }

    #[test]
    fn set_content_silent_keeps_updated_at_and_load_all_includes_trashed() {
        let s = store();
        s.save_note(&note("a", "<p>v1</p>", 1000)).unwrap();
        s.trash_note("a", 1).unwrap();
        s.set_content_silent("a", "<p>v2</p>").unwrap();
        let all = s.load_all_notes().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].content, "<p>v2</p>");
        assert_eq!(all[0].updated_at, 1000);
    }

    #[test]
    fn purge_trashed_respects_threshold_then_all() {
        let s = store();
        s.save_note(&note("old", "<p>o</p>", 1)).unwrap();
        s.save_note(&note("new", "<p>n</p>", 1)).unwrap();
        s.trash_note("old", 100).unwrap();
        s.trash_note("new", 1000).unwrap();
        s.purge_trashed(Some(500)).unwrap();
        let t = s.load_trashed().unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].id, "new");
        s.purge_trashed(None).unwrap();
        assert!(s.load_trashed().unwrap().is_empty());
    }
}
