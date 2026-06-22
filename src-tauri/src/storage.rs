use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// A single note. `updated_at` is epoch milliseconds.
/// `rename_all = camelCase` makes the JSON `{ id, content, updatedAt }`,
/// matching the frontend `Note` type and the legacy dginx-notes JSON files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub content: String,
    pub updated_at: i64,
}

/// Owns the SQLite connection. Isolated so a future sync client can build on it.
pub struct Store {
    pub conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        Ok(Self {
            conn: Connection::open(path)?,
        })
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Ok(Self {
            conn: Connection::open_in_memory()?,
        })
    }

    /// All notes, newest first.
    pub fn load_notes(&self) -> rusqlite::Result<Vec<Note>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, content, updated_at FROM notes ORDER BY updated_at DESC")?;
        let rows = stmt.query_map([], |r| {
            Ok(Note {
                id: r.get(0)?,
                content: r.get(1)?,
                updated_at: r.get(2)?,
            })
        })?;
        rows.collect()
    }

    /// Insert or update a note by id.
    pub fn save_note(&self, note: &Note) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT INTO notes (id, content, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at",
            (&note.id, &note.content, note.updated_at),
        )?;
        Ok(())
    }

    pub fn delete_note(&self, id: &str) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM notes WHERE id = ?1", [id])?;
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
        Note { id: id.into(), content: content.into(), updated_at }
    }

    #[test]
    fn loads_empty_when_no_notes() {
        let s = store();
        assert_eq!(s.load_notes().unwrap(), vec![]);
    }

    #[test]
    fn saves_and_loads_a_note() {
        let s = store();
        s.save_note(&note("a", "<p>hi</p>", 1000)).unwrap();
        let notes = s.load_notes().unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0], note("a", "<p>hi</p>", 1000));
    }

    #[test]
    fn loads_notes_newest_first() {
        let s = store();
        s.save_note(&note("old", "<p>old</p>", 1000)).unwrap();
        s.save_note(&note("new", "<p>new</p>", 2000)).unwrap();
        let ids: Vec<String> = s.load_notes().unwrap().into_iter().map(|n| n.id).collect();
        assert_eq!(ids, vec!["new", "old"]);
    }

    #[test]
    fn save_upserts_existing_note() {
        let s = store();
        s.save_note(&note("a", "<p>v1</p>", 1000)).unwrap();
        s.save_note(&note("a", "<p>v2</p>", 1500)).unwrap();
        let notes = s.load_notes().unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].content, "<p>v2</p>");
        assert_eq!(notes[0].updated_at, 1500);
    }

    #[test]
    fn deletes_only_the_target_note() {
        let s = store();
        s.save_note(&note("a", "<p>a</p>", 1000)).unwrap();
        s.save_note(&note("b", "<p>b</p>", 2000)).unwrap();
        s.delete_note("a").unwrap();
        let ids: Vec<String> = s.load_notes().unwrap().into_iter().map(|n| n.id).collect();
        assert_eq!(ids, vec!["b"]);
    }
}
