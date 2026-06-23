use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Revision {
    pub id: i64,
    pub note_id: String,
    pub created_at: i64,
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Append a revision unless content is empty or identical to the latest one,
/// then prune to the newest `limit` for that note.
pub fn add_revision(conn: &Connection, note_id: &str, content: &str, limit: i64) -> rusqlite::Result<()> {
    let trimmed = content.trim();
    if trimmed.is_empty() || trimmed == "<p></p>" {
        return Ok(());
    }
    let latest: Option<String> = conn
        .query_row(
            "SELECT content FROM note_revisions WHERE note_id = ?1 ORDER BY created_at DESC, id DESC LIMIT 1",
            [note_id],
            |r| r.get(0),
        )
        .optional()?;
    if latest.as_deref().map(str::trim) == Some(trimmed) {
        return Ok(());
    }
    conn.execute(
        "INSERT INTO note_revisions (note_id, content, created_at) VALUES (?1, ?2, ?3)",
        (note_id, content, now_ms()),
    )?;
    conn.execute(
        "DELETE FROM note_revisions WHERE note_id = ?1 AND id NOT IN (SELECT id FROM note_revisions WHERE note_id = ?1 ORDER BY created_at DESC, id DESC LIMIT ?2)",
        (note_id, limit.max(1)),
    )?;
    Ok(())
}

pub fn list_revisions(conn: &Connection, note_id: &str) -> rusqlite::Result<Vec<Revision>> {
    let mut stmt = conn.prepare("SELECT id, note_id, created_at FROM note_revisions WHERE note_id = ?1 ORDER BY created_at DESC, id DESC")?;
    let rows = stmt.query_map([note_id], |r| Ok(Revision { id: r.get(0)?, note_id: r.get(1)?, created_at: r.get(2)? }))?;
    rows.collect()
}

pub fn revision_content(conn: &Connection, id: i64) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT content FROM note_revisions WHERE id = ?1", [id], |r| r.get(0)).optional()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate, storage::Store};

    fn store() -> Store {
        let s = Store::open_in_memory().unwrap();
        migrate::run_migrations(&s.conn).unwrap();
        s
    }

    #[test]
    fn add_dedups_and_prunes() {
        let s = store();
        add_revision(&s.conn, "n", "<p>a</p>", 3).unwrap();
        add_revision(&s.conn, "n", "<p>a</p>", 3).unwrap();
        assert_eq!(list_revisions(&s.conn, "n").unwrap().len(), 1);
        add_revision(&s.conn, "n", "<p>b</p>", 3).unwrap();
        add_revision(&s.conn, "n", "<p>c</p>", 3).unwrap();
        add_revision(&s.conn, "n", "<p>d</p>", 3).unwrap();
        assert_eq!(list_revisions(&s.conn, "n").unwrap().len(), 3);
    }

    #[test]
    fn skips_empty() {
        let s = store();
        add_revision(&s.conn, "n", "", 5).unwrap();
        add_revision(&s.conn, "n", "<p></p>", 5).unwrap();
        assert!(list_revisions(&s.conn, "n").unwrap().is_empty());
    }

    #[test]
    fn content_roundtrip() {
        let s = store();
        add_revision(&s.conn, "n", "<p>hello</p>", 5).unwrap();
        let id = list_revisions(&s.conn, "n").unwrap()[0].id;
        assert_eq!(revision_content(&s.conn, id).unwrap().as_deref(), Some("<p>hello</p>"));
    }
}
