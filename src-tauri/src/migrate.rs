use rusqlite::Connection;

pub fn run_migrations(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
    )?;
    let version: i64 = get_meta(conn, "schema_version")?
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    if version < 1 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS notes (
                id TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                updated_at INTEGER NOT NULL
            );",
        )?;
        set_meta(conn, "schema_version", "1")?;
    }

    if version < 2 {
        conn.execute_batch(
            "ALTER TABLE notes ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0;
             CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )?;
        set_meta(conn, "schema_version", "2")?;
    }

    if version < 3 {
        conn.execute_batch(
            "ALTER TABLE notes ADD COLUMN archived INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE notes ADD COLUMN color TEXT NOT NULL DEFAULT '';",
        )?;
        set_meta(conn, "schema_version", "3")?;
    }

    Ok(())
}

pub fn get_meta(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    use rusqlite::OptionalExtension;
    conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
        .optional()
}

pub fn set_meta(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (key, value),
    )?;
    Ok(())
}

use std::path::Path;

use crate::storage::{Note, Store};

/// One-time import of legacy dginx-notes JSON files (one `<id>.json` per note,
/// shape `{ id, content, updatedAt }`). Idempotent via the `legacy_imported`
/// meta flag. Missing dir is a clean no-op. Returns the number imported.
pub fn import_legacy_if_needed(store: &Store, legacy_dir: &Path) -> rusqlite::Result<usize> {
    if get_meta(&store.conn, "legacy_imported")?.is_some() {
        return Ok(0);
    }
    let mut imported = 0usize;
    if legacy_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(legacy_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Ok(raw) = std::fs::read_to_string(&path) {
                        if let Ok(note) = serde_json::from_str::<Note>(&raw) {
                            store.save_note(&note)?;
                            imported += 1;
                        }
                    }
                }
            }
        }
    }
    set_meta(&store.conn, "legacy_imported", "1")?;
    Ok(imported)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Store;

    fn store() -> Store {
        let s = Store::open_in_memory().unwrap();
        run_migrations(&s.conn).unwrap();
        s
    }

    #[test]
    fn migration_sets_schema_version() {
        let s = store();
        assert_eq!(get_meta(&s.conn, "schema_version").unwrap().as_deref(), Some("3"));
    }

    #[test]
    fn migration_is_idempotent() {
        let s = store();
        run_migrations(&s.conn).unwrap();
        assert_eq!(get_meta(&s.conn, "schema_version").unwrap().as_deref(), Some("3"));
    }

    #[test]
    fn import_reads_legacy_json_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("n1.json"),
            r#"{"id":"n1","content":"<p>legacy</p>","updatedAt":1234}"#,
        ).unwrap();
        let s = store();
        let count = import_legacy_if_needed(&s, dir.path()).unwrap();
        assert_eq!(count, 1);
        let notes = s.load_notes().unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id, "n1");
        assert_eq!(notes[0].updated_at, 1234);
    }

    #[test]
    fn import_is_skipped_on_second_run() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("n1.json"),
            r#"{"id":"n1","content":"<p>x</p>","updatedAt":1}"#,
        ).unwrap();
        let s = store();
        assert_eq!(import_legacy_if_needed(&s, dir.path()).unwrap(), 1);
        assert_eq!(import_legacy_if_needed(&s, dir.path()).unwrap(), 0);
    }

    #[test]
    fn import_missing_dir_is_noop() {
        let s = store();
        let count = import_legacy_if_needed(&s, Path::new("/nonexistent/path/xyz")).unwrap();
        assert_eq!(count, 0);
    }
}
