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

    if version < 4 {
        conn.execute_batch("ALTER TABLE notes ADD COLUMN due_at INTEGER;")?;
        set_meta(conn, "schema_version", "4")?;
    }

    if version < 5 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS folders (
                id         TEXT PRIMARY KEY,
                name       TEXT NOT NULL,
                parent_id  TEXT,
                position   INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL
            );
            ALTER TABLE notes ADD COLUMN folder_id TEXT;",
        )?;
        set_meta(conn, "schema_version", "5")?;
    }

    if version < 6 {
        conn.execute_batch(
            "ALTER TABLE notes ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
             UPDATE notes SET position = -updated_at;",
        )?;
        set_meta(conn, "schema_version", "6")?;
    }

    if version < 7 {
        conn.execute_batch(
            "ALTER TABLE folders ADD COLUMN icon  TEXT NOT NULL DEFAULT '';
             ALTER TABLE folders ADD COLUMN color TEXT NOT NULL DEFAULT '';",
        )?;
        set_meta(conn, "schema_version", "7")?;
    }

    if version < 8 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS note_revisions (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                note_id    TEXT NOT NULL,
                content    TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_note_revisions_note ON note_revisions (note_id, created_at DESC);",
        )?;
        set_meta(conn, "schema_version", "8")?;
    }

    if version < 9 {
        conn.execute_batch("ALTER TABLE folders ADD COLUMN sort TEXT NOT NULL DEFAULT 'manual';")?;
        set_meta(conn, "schema_version", "9")?;
    }

    if version < 10 {
        conn.execute_batch("ALTER TABLE notes ADD COLUMN deleted_at INTEGER;")?;
        set_meta(conn, "schema_version", "10")?;
    }

    if version < 11 {
        // C1 sync: dirty-flag write queue + folder sync metadata.
        // Backfill folders.updated_at so a first push isn't infinitely old.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        conn.execute_batch(
            "ALTER TABLE notes   ADD COLUMN dirty      INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE folders ADD COLUMN updated_at INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE folders ADD COLUMN deleted_at INTEGER;
             ALTER TABLE folders ADD COLUMN dirty      INTEGER NOT NULL DEFAULT 0;",
        )?;
        conn.execute(
            "UPDATE folders SET updated_at = ?1 WHERE updated_at = 0",
            [now],
        )?;
        set_meta(conn, "schema_version", "11")?;
    }

    if version < 12 {
        conn.execute_batch(
            "ALTER TABLE notes ADD COLUMN protected INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE folders ADD COLUMN locked INTEGER NOT NULL DEFAULT 0;
             CREATE TABLE IF NOT EXISTS vault (
                 id         INTEGER PRIMARY KEY CHECK (id = 1),
                 record     TEXT NOT NULL,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );",
        )?;
        set_meta(conn, "schema_version", "12")?;
    }

    if version < 13 {
        // Plaintext note title (schema v13) — kept visible in the list even
        // when `content` is sealed ciphertext for a protected note (only the
        // BODY is secret in this feature's threat model).
        conn.execute_batch("ALTER TABLE notes ADD COLUMN title TEXT NOT NULL DEFAULT '';")?;
        // Backfill from existing plaintext content. Protected notes already
        // hold ciphertext at this point — deriving a "title" from that would
        // just be garbage, so they're left with title = '' and pick one up on
        // their next save or re-protect.
        let mut stmt = conn.prepare("SELECT id, content FROM notes WHERE protected = 0")?;
        let rows: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        drop(stmt);
        for (id, content) in rows {
            let title = crate::storage::note_preview(&content);
            conn.execute("UPDATE notes SET title = ?2 WHERE id = ?1", (&id, &title))?;
        }
        set_meta(conn, "schema_version", "13")?;
    }

    if version < 14 {
        // "Hide from MCP" (schema v14) — a LOCAL-only opt-out flag, independent
        // of the protected-notes vault: a note or folder marked here is never
        // returned or writable via the local MCP server, whether or not it's
        // also protected. Never synced (see `sync::note_to_wire`/
        // `folder_to_wire`, which deliberately omit it).
        conn.execute_batch(
            "ALTER TABLE notes   ADD COLUMN mcp_hidden INTEGER NOT NULL DEFAULT 0;
             ALTER TABLE folders ADD COLUMN mcp_hidden INTEGER NOT NULL DEFAULT 0;",
        )?;
        set_meta(conn, "schema_version", "14")?;
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

pub fn get_meta_i64(conn: &Connection, key: &str, default: i64) -> i64 {
    get_meta(conn, key)
        .ok()
        .flatten()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

pub fn set_meta_i64(conn: &Connection, key: &str, value: i64) -> rusqlite::Result<()> {
    set_meta(conn, key, &value.to_string())
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
        assert_eq!(
            get_meta(&s.conn, "schema_version").unwrap().as_deref(),
            Some("14")
        );
    }

    #[test]
    fn migration_is_idempotent() {
        let s = store();
        run_migrations(&s.conn).unwrap();
        assert_eq!(
            get_meta(&s.conn, "schema_version").unwrap().as_deref(),
            Some("14")
        );
    }

    #[test]
    fn migration_v14_adds_mcp_hidden_columns_defaulting_false() {
        let s = store();
        s.save_note(&Note {
            id: "n".into(),
            content: "<p>x</p>".into(),
            updated_at: 1,
            ..Default::default()
        })
        .unwrap();
        crate::folders::create_folder(&s.conn, "f", "F", None).unwrap();
        assert!(!s.note_mcp_hidden("n").unwrap());
        assert!(!s.folder_mcp_hidden("f").unwrap());
        s.set_note_mcp_hidden("n", true).unwrap();
        s.set_folder_mcp_hidden("f", true).unwrap();
        assert!(s.note_mcp_hidden("n").unwrap());
        assert!(s.folder_mcp_hidden("f").unwrap());
    }

    #[test]
    fn migration_v13_adds_and_backfills_title() {
        // Simulate a pre-v13 database (schema_version = 12, no `title` column
        // yet) with one plaintext and one already-protected note, then run
        // the real migration path and check the backfill it performs.
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE notes (
                 id         TEXT PRIMARY KEY,
                 content    TEXT NOT NULL,
                 updated_at INTEGER NOT NULL,
                 pinned     INTEGER NOT NULL DEFAULT 0,
                 archived   INTEGER NOT NULL DEFAULT 0,
                 color      TEXT NOT NULL DEFAULT '',
                 due_at     INTEGER,
                 folder_id  TEXT,
                 position   INTEGER NOT NULL DEFAULT 0,
                 deleted_at INTEGER,
                 dirty      INTEGER NOT NULL DEFAULT 0,
                 protected  INTEGER NOT NULL DEFAULT 0
             );
             -- Minimal stand-in so the v14 step below (`ALTER TABLE folders
             -- ADD COLUMN mcp_hidden`), which now also runs from this
             -- synthetic v12 baseline, has a table to alter.
             CREATE TABLE folders (id TEXT PRIMARY KEY);",
        )
        .unwrap();
        set_meta(&conn, "schema_version", "12").unwrap();
        conn.execute(
            "INSERT INTO notes (id, content, updated_at) VALUES ('plain', '<p>Plain Title</p><p>body</p>', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO notes (id, content, updated_at, protected) VALUES ('secret', 'ciphertext-blob', 1, 1)",
            [],
        )
        .unwrap();

        run_migrations(&conn).unwrap();

        assert_eq!(
            get_meta(&conn, "schema_version").unwrap().as_deref(),
            Some("14")
        );
        let title_of = |id: &str| -> String {
            conn.query_row("SELECT title FROM notes WHERE id = ?1", [id], |r| r.get(0))
                .unwrap()
        };
        assert_eq!(title_of("plain"), "Plain Title");
        assert_eq!(
            title_of("secret"),
            "",
            "a protected note's ciphertext can't be backfilled into a title"
        );
    }

    #[test]
    fn import_reads_legacy_json_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("n1.json"),
            r#"{"id":"n1","content":"<p>legacy</p>","updatedAt":1234}"#,
        )
        .unwrap();
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
        )
        .unwrap();
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
