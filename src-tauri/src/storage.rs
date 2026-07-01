use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
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
    #[serde(default)]
    pub dirty: bool,
}

pub struct Store {
    pub conn: Connection,
    pub sync_enabled: bool,
}

const COLS: &str = "id, content, updated_at, pinned, archived, color, due_at, folder_id, position, deleted_at, dirty";

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn row_to_note(r: &rusqlite::Row) -> rusqlite::Result<Note> {
    Ok(Note {
        id: r.get(0)?,
        content: r.get(1)?,
        updated_at: r.get(2)?,
        pinned: r.get(3)?,
        archived: r.get(4)?,
        color: r.get(5)?,
        due_at: r.get(6)?,
        folder_id: r.get(7)?,
        position: r.get(8)?,
        deleted_at: r.get(9)?,
        dirty: r.get(10)?,
    })
}

/// Lightweight list item: every note field except the (potentially huge) HTML
/// content, plus a short preview and task counts computed from it. Lets the note
/// list load without holding/shipping full content — that comes on demand.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NoteMeta {
    pub id: String,
    pub updated_at: i64,
    pub pinned: bool,
    pub archived: bool,
    pub color: String,
    pub due_at: Option<i64>,
    pub folder_id: Option<String>,
    pub position: i64,
    pub deleted_at: Option<i64>,
    pub dirty: bool,
    pub preview: String,
    pub tasks_done: i64,
    pub tasks_total: i64,
}

/// A search match: the note's list metadata plus a snippet around the hit.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub note: NoteMeta,
    pub snippet: String,
}

/// Plain text of an HTML fragment: strip tags (space at each `<`), collapse ws.
fn strip_tags(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => {
                in_tag = true;
                if !text.is_empty() && !text.ends_with(' ') {
                    text.push(' ');
                }
            }
            '>' => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Short plain-text title of a note's HTML — the text of the FIRST element only
/// (mirrors the JS `getPreview`'s `firstElementChild.textContent`), first 60
/// chars. Empty content yields "" (the UI supplies a localized fallback).
pub fn note_preview(html: &str) -> String {
    let s = html.trim_start();
    let inner = match s.strip_prefix('<') {
        Some(rest) => {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            match (name.is_empty(), s.find('>')) {
                (false, Some(gt)) => {
                    let body = &s[gt + 1..];
                    let close = format!("</{}", name.to_lowercase());
                    let end = body.to_lowercase().find(&close).unwrap_or(body.len());
                    body[..end].to_string()
                }
                _ => s.to_string(),
            }
        }
        None => s.to_string(),
    };
    strip_tags(&inner).chars().take(60).collect()
}

/// Count Tiptap task-list items: total `data-checked="…"`, done where value is
/// "true". Mirrors the JS `countTasks`.
pub fn task_counts(html: &str) -> (i64, i64) {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| regex::Regex::new(r#"data-checked="([^"]*)""#).unwrap());
    let mut total = 0;
    let mut done = 0;
    for cap in re.captures_iter(html) {
        total += 1;
        if &cap[1] == "true" {
            done += 1;
        }
    }
    (done, total)
}

fn row_to_meta(r: &rusqlite::Row) -> rusqlite::Result<NoteMeta> {
    let content: String = r.get(1)?;
    let (tasks_done, tasks_total) = task_counts(&content);
    Ok(NoteMeta {
        id: r.get(0)?,
        updated_at: r.get(2)?,
        pinned: r.get(3)?,
        archived: r.get(4)?,
        color: r.get(5)?,
        due_at: r.get(6)?,
        folder_id: r.get(7)?,
        position: r.get(8)?,
        deleted_at: r.get(9)?,
        dirty: r.get(10)?,
        preview: note_preview(&content),
        tasks_done,
        tasks_total,
    })
}

/// A window of plain text around the first case-insensitive match of `q_lower`
/// (already lowercased), with ellipses. Mirrors the JS `snippetAround`.
fn snippet_around(text: &str, q_lower: &str) -> String {
    let tl: Vec<char> = text.chars().collect();
    let lower = text.to_lowercase();
    match lower.find(q_lower) {
        None => tl.iter().take(80).collect::<String>().trim().to_string(),
        Some(byte_idx) => {
            let char_idx = lower[..byte_idx].chars().count();
            let start = char_idx.saturating_sub(30);
            let end = (start + 80).min(tl.len());
            let mut s = String::new();
            if start > 0 {
                s.push('…');
            }
            s.push_str(tl[start..end].iter().collect::<String>().trim());
            if end < tl.len() {
                s.push('…');
            }
            s
        }
    }
}

impl Store {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        Ok(Self {
            conn: Connection::open(path)?,
            sync_enabled: false,
        })
    }

    #[cfg(test)]
    pub fn open_in_memory() -> rusqlite::Result<Self> {
        Ok(Self {
            conn: Connection::open_in_memory()?,
            sync_enabled: false,
        })
    }

    pub fn load_notes(&self) -> rusqlite::Result<Vec<Note>> {
        let sql = format!(
            "SELECT {COLS} FROM notes WHERE deleted_at IS NULL ORDER BY pinned DESC, position ASC"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], row_to_note)?;
        rows.collect()
    }

    /// List metadata for active notes — same order as `load_notes`, but content
    /// is reduced to a preview + task counts and never returned.
    pub fn load_notes_meta(&self) -> rusqlite::Result<Vec<NoteMeta>> {
        let sql = format!(
            "SELECT {COLS} FROM notes WHERE deleted_at IS NULL ORDER BY pinned DESC, position ASC"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], row_to_meta)?;
        rows.collect()
    }

    /// List metadata for trashed notes (newest-deleted first).
    pub fn load_trashed_meta(&self) -> rusqlite::Result<Vec<NoteMeta>> {
        let sql = format!(
            "SELECT {COLS} FROM notes WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], row_to_meta)?;
        rows.collect()
    }

    /// The full HTML content of one note, or `None` if it doesn't exist.
    pub fn load_note_content(&self, id: &str) -> rusqlite::Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT content FROM notes WHERE id = ?1")?;
        let mut rows = stmt.query_map([id], |r| r.get::<_, String>(0))?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    /// Case-insensitive full-text search over active notes (title/preview first,
    /// then body). Returns list metadata + a snippet for each hit.
    pub fn search_notes(&self, query: &str, limit: usize) -> rusqlite::Result<Vec<SearchHit>> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Ok(vec![]);
        }
        let sql = format!(
            "SELECT {COLS} FROM notes WHERE deleted_at IS NULL AND archived = 0 ORDER BY pinned DESC, position ASC"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |r| {
            let content: String = r.get(1)?;
            Ok((row_to_meta(r)?, content))
        })?;
        let mut title_hits = Vec::new();
        let mut body_hits = Vec::new();
        for row in rows {
            let (meta, content) = row?;
            let plain = crate::stats::strip_html(&content);
            let in_title = meta.preview.to_lowercase().contains(&q);
            let in_body = plain.to_lowercase().contains(&q);
            if !in_title && !in_body {
                continue;
            }
            let snippet = snippet_around(&plain, &q);
            let hit = SearchHit {
                note: meta,
                snippet,
            };
            if in_title {
                title_hits.push(hit);
            } else {
                body_hits.push(hit);
            }
        }
        title_hits.extend(body_hits);
        title_hits.truncate(limit);
        Ok(title_hits)
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
        self.conn
            .execute("UPDATE notes SET content = ?2 WHERE id = ?1", (id, content))?;
        Ok(())
    }

    pub fn save_note(&self, note: &Note) -> rusqlite::Result<()> {
        let (updated_at, dirty) = if self.sync_enabled {
            (now_ms(), 1)
        } else {
            (note.updated_at, 0)
        };
        self.conn.execute(
            "INSERT INTO notes (id, content, updated_at, pinned, archived, color, due_at, folder_id, position, dirty)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at, dirty = excluded.dirty",
            (&note.id, &note.content, updated_at, note.pinned, note.archived, &note.color, note.due_at, &note.folder_id, note.position, dirty),
        )?;
        Ok(())
    }

    pub fn delete_note(&self, id: &str) -> rusqlite::Result<()> {
        self.conn
            .execute("DELETE FROM note_revisions WHERE note_id = ?1", [id])?;
        self.conn.execute("DELETE FROM notes WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Server-context delete: tombstone + mark dirty (pushed as a tombstone).
    pub fn sync_delete_note(&self, id: &str) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE notes SET deleted_at = ?2, dirty = 1, updated_at = ?2 WHERE id = ?1",
            (id, now_ms()),
        )?;
        Ok(())
    }

    pub fn load_dirty_notes(&self) -> rusqlite::Result<Vec<Note>> {
        let sql = format!("SELECT {COLS} FROM notes WHERE dirty = 1");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], row_to_note)?;
        rows.collect()
    }

    /// Clear the dirty flag for rows that were just pushed — but only if the row
    /// hasn't been re-edited since we snapshotted it (`updated_at` still matches).
    /// An edit landing during the push/pull network window bumps `updated_at`, so
    /// it stays queued and is pushed next cycle instead of being silently dropped.
    pub fn clear_note_dirty(&self, rows: &[(String, i64)]) -> rusqlite::Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for (id, updated_at) in rows {
            tx.execute(
                "UPDATE notes SET dirty = 0 WHERE id = ?1 AND updated_at = ?2",
                (id, updated_at),
            )?;
        }
        tx.commit()
    }

    pub fn set_pinned(&self, id: &str, pinned: bool) -> rusqlite::Result<()> {
        self.conn
            .execute("UPDATE notes SET pinned = ?2 WHERE id = ?1", (id, pinned))?;
        if self.sync_enabled {
            self.conn.execute(
                "UPDATE notes SET updated_at = ?2, dirty = 1 WHERE id = ?1",
                (id, now_ms()),
            )?;
        }
        Ok(())
    }

    pub fn set_archived(&self, id: &str, archived: bool) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE notes SET archived = ?2 WHERE id = ?1",
            (id, archived),
        )?;
        if self.sync_enabled {
            self.conn.execute(
                "UPDATE notes SET updated_at = ?2, dirty = 1 WHERE id = ?1",
                (id, now_ms()),
            )?;
        }
        Ok(())
    }

    pub fn set_color(&self, id: &str, color: &str) -> rusqlite::Result<()> {
        self.conn
            .execute("UPDATE notes SET color = ?2 WHERE id = ?1", (id, color))?;
        if self.sync_enabled {
            self.conn.execute(
                "UPDATE notes SET updated_at = ?2, dirty = 1 WHERE id = ?1",
                (id, now_ms()),
            )?;
        }
        Ok(())
    }

    /// Set or clear the due date. Does NOT touch `updated_at`.
    pub fn set_due(&self, id: &str, due_at: Option<i64>) -> rusqlite::Result<()> {
        self.conn
            .execute("UPDATE notes SET due_at = ?2 WHERE id = ?1", (id, due_at))?;
        if self.sync_enabled {
            self.conn.execute(
                "UPDATE notes SET updated_at = ?2, dirty = 1 WHERE id = ?1",
                (id, now_ms()),
            )?;
        }
        Ok(())
    }

    /// Move a note to a folder (None = root). Does NOT touch `updated_at`.
    pub fn set_folder(&self, id: &str, folder_id: Option<&str>) -> rusqlite::Result<()> {
        self.conn.execute(
            "UPDATE notes SET folder_id = ?2 WHERE id = ?1",
            (id, folder_id),
        )?;
        if self.sync_enabled {
            self.conn.execute(
                "UPDATE notes SET updated_at = ?2, dirty = 1 WHERE id = ?1",
                (id, now_ms()),
            )?;
        }
        Ok(())
    }

    /// Set folder + position for each id in the given order.
    pub fn reorder_notes(&self, folder_id: Option<&str>, ids: &[String]) -> rusqlite::Result<()> {
        for (i, id) in ids.iter().enumerate() {
            self.conn.execute(
                "UPDATE notes SET folder_id = ?2, position = ?3 WHERE id = ?1",
                (id, folder_id, i as i64),
            )?;
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
        self.conn
            .execute("UPDATE notes SET deleted_at = ?2 WHERE id = ?1", (id, ts))?;
        Ok(())
    }

    pub fn restore_note(&self, id: &str) -> rusqlite::Result<()> {
        self.conn
            .execute("UPDATE notes SET deleted_at = NULL WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn purge_trashed(&self, before: Option<i64>) -> rusqlite::Result<()> {
        match before {
            Some(t) => {
                self.conn.execute("DELETE FROM note_revisions WHERE note_id IN (SELECT id FROM notes WHERE deleted_at IS NOT NULL AND deleted_at < ?1)", [t])?;
                self.conn.execute(
                    "DELETE FROM notes WHERE deleted_at IS NOT NULL AND deleted_at < ?1",
                    [t],
                )?;
            }
            None => {
                self.conn.execute("DELETE FROM note_revisions WHERE note_id IN (SELECT id FROM notes WHERE deleted_at IS NOT NULL)", [])?;
                self.conn
                    .execute("DELETE FROM notes WHERE deleted_at IS NOT NULL", [])?;
            }
        }
        Ok(())
    }
}

/// Upsert a server note against an arbitrary connection (used inside a tx).
/// Last-write-wins: an existing row is overwritten only when the incoming
/// server version is newer-or-equal (`updated_at`), so a local edit made during
/// the sync window (which has a strictly greater `updated_at`) is preserved and
/// pushed on the next cycle.
pub fn upsert_note_from_server_conn(conn: &rusqlite::Connection, n: &Note) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO notes (id, content, updated_at, pinned, archived, color, due_at, folder_id, position, deleted_at, dirty)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0)
         ON CONFLICT(id) DO UPDATE SET
            content=excluded.content, updated_at=excluded.updated_at, pinned=excluded.pinned,
            archived=excluded.archived, color=excluded.color, due_at=excluded.due_at,
            folder_id=excluded.folder_id, position=excluded.position, deleted_at=excluded.deleted_at, dirty=0
         WHERE excluded.updated_at >= notes.updated_at",
        (&n.id, &n.content, n.updated_at, n.pinned, n.archived, &n.color, n.due_at, &n.folder_id, n.position, n.deleted_at),
    )?;
    Ok(())
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

    fn mem() -> Store {
        let s = Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        s
    }

    fn note(id: &str, content: &str, updated_at: i64) -> Note {
        Note {
            id: id.into(),
            content: content.into(),
            updated_at,
            pinned: false,
            archived: false,
            color: String::new(),
            due_at: None,
            folder_id: None,
            position: 0,
            deleted_at: None,
            dirty: false,
        }
    }

    #[test]
    fn loads_empty_when_no_notes() {
        assert_eq!(store().load_notes().unwrap(), vec![]);
    }

    #[test]
    fn note_preview_strips_tags_and_truncates() {
        assert_eq!(note_preview("<p>Hello <b>world</b></p>"), "Hello world");
        assert_eq!(note_preview(""), "");
        assert_eq!(note_preview("<p></p>"), "");
        assert_eq!(
            note_preview(&format!("<p>{}</p>", "x".repeat(100))).len(),
            60
        );
    }

    #[test]
    fn task_counts_counts_checked_items() {
        let html = r#"<li data-checked="true">a</li><li data-checked="false">b</li><li data-checked="true">c</li>"#;
        assert_eq!(task_counts(html), (2, 3));
        assert_eq!(task_counts("<p>no tasks</p>"), (0, 0));
    }

    #[test]
    fn load_notes_meta_has_preview_and_counts_no_content() {
        let s = store();
        s.save_note(&note(
            "a",
            r#"<p>Title</p><li data-checked="true">x</li>"#,
            1000,
        ))
        .unwrap();
        let meta = s.load_notes_meta().unwrap();
        assert_eq!(meta.len(), 1);
        assert_eq!(meta[0].id, "a");
        assert_eq!(meta[0].preview, "Title"); // first element only, like JS getPreview
        assert_eq!((meta[0].tasks_done, meta[0].tasks_total), (1, 1));
    }

    #[test]
    fn load_note_content_returns_html_or_none() {
        let s = store();
        s.save_note(&note("a", "<p>body</p>", 1000)).unwrap();
        assert_eq!(
            s.load_note_content("a").unwrap().as_deref(),
            Some("<p>body</p>")
        );
        assert_eq!(s.load_note_content("missing").unwrap(), None);
    }

    #[test]
    fn search_notes_ranks_title_first_and_snippets() {
        let s = store();
        // body-only match
        s.save_note(&note("body", "<p>Zeta</p><p>the apple is red</p>", 10))
            .unwrap();
        // title match (higher rank)
        s.save_note(&note("title", "<p>apple crumble</p>", 20))
            .unwrap();
        let hits = s.search_notes("apple", 50).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].note.id, "title"); // title/preview hit ranks first
        assert!(hits[1].snippet.to_lowercase().contains("apple"));
        assert!(s.search_notes("", 50).unwrap().is_empty());
        assert!(s.search_notes("nomatch", 50).unwrap().is_empty());
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
        s.reorder_notes(Some("f1"), &["b".to_string(), "a".to_string()])
            .unwrap();
        let loaded = s.load_notes().unwrap();
        // both now in f1, ordered b(pos0) then a(pos1)
        assert_eq!(
            loaded.iter().map(|n| n.id.clone()).collect::<Vec<_>>(),
            vec!["b", "a"]
        );
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
        let ids: Vec<String> = s
            .recent_notes(5)
            .unwrap()
            .into_iter()
            .map(|n| n.id)
            .collect();
        assert_eq!(ids, vec!["a"]);
    }

    #[test]
    fn trash_hides_from_load_and_shows_in_trashed() {
        let s = store();
        s.save_note(&note("a", "<p>a</p>", 1)).unwrap();
        s.trash_note("a", 1000).unwrap();
        assert!(s.load_notes().unwrap().is_empty());
        let t = s.load_trashed_meta().unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].deleted_at, Some(1000));
        s.restore_note("a").unwrap();
        assert_eq!(s.load_notes().unwrap().len(), 1);
        assert!(s.load_trashed_meta().unwrap().is_empty());
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
        let t = s.load_trashed_meta().unwrap();
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].id, "new");
        s.purge_trashed(None).unwrap();
        assert!(s.load_trashed_meta().unwrap().is_empty());
    }

    #[test]
    fn sync_enabled_save_marks_dirty_and_bumps_updated_at() {
        let mut s = mem();
        s.sync_enabled = true;
        let n = Note {
            id: "n1".into(),
            content: "<p>a</p>".into(),
            updated_at: 1,
            ..Default::default()
        };
        s.save_note(&n).unwrap();
        let saved = &s.load_notes().unwrap()[0];
        assert!(saved.dirty);
        assert!(saved.updated_at > 1); // bumped to ~now
    }

    #[test]
    fn sync_disabled_save_leaves_clean() {
        let s = mem();
        let n = Note {
            id: "n1".into(),
            content: "<p>a</p>".into(),
            updated_at: 5,
            ..Default::default()
        };
        s.save_note(&n).unwrap();
        let saved = &s.load_notes().unwrap()[0];
        assert!(!saved.dirty);
        assert_eq!(saved.updated_at, 5);
    }

    #[test]
    fn sync_delete_tombstones_instead_of_removing() {
        let mut s = mem();
        s.sync_enabled = true;
        s.save_note(&Note {
            id: "n1".into(),
            content: "x".into(),
            updated_at: 1,
            ..Default::default()
        })
        .unwrap();
        s.sync_delete_note("n1").unwrap();
        assert!(s.load_notes().unwrap().is_empty());
        let all = s.load_all_notes().unwrap();
        assert_eq!(all.len(), 1);
        assert!(all[0].deleted_at.is_some() && all[0].dirty);
    }

    #[test]
    fn dirty_collect_and_clear() {
        let mut s = mem();
        s.sync_enabled = true;
        s.save_note(&Note {
            id: "n1".into(),
            content: "x".into(),
            updated_at: 1,
            ..Default::default()
        })
        .unwrap();
        let dirty = s.load_dirty_notes().unwrap();
        assert_eq!(dirty.len(), 1);
        s.clear_note_dirty(&[("n1".into(), dirty[0].updated_at)])
            .unwrap();
        assert!(s.load_dirty_notes().unwrap().is_empty());
    }

    #[test]
    fn concurrent_edit_during_sync_is_not_dropped() {
        // Reproduces the edit-during-network-window race: a row is snapshotted
        // for push, then re-edited before the dirty-clear / pull-apply land.
        let mut s = mem();
        s.sync_enabled = true;
        s.save_note(&Note {
            id: "n1".into(),
            content: "A".into(),
            updated_at: 1,
            ..Default::default()
        })
        .unwrap();
        let pushed = s.load_dirty_notes().unwrap()[0].clone(); // snapshot (updated_at = T1)

        // The user edits during the network window: content "B", updated_at bumps to T2 > T1.
        s.set_content_silent("n1", "B").unwrap();
        s.conn
            .execute(
                "UPDATE notes SET updated_at = ?2, dirty = 1 WHERE id = ?1",
                ("n1", pushed.updated_at + 5),
            )
            .unwrap();

        // Post-sync clear uses the STALE snapshot — must NOT clear the re-edited row.
        s.clear_note_dirty(&[("n1".into(), pushed.updated_at)])
            .unwrap();
        assert_eq!(
            s.load_dirty_notes().unwrap().len(),
            1,
            "re-edited row stays queued"
        );

        // Pull applies the OLDER server row (content "A", updated_at T1) — LWW must keep "B".
        upsert_note_from_server_conn(
            &s.conn,
            &Note {
                id: "n1".into(),
                content: "A".into(),
                updated_at: pushed.updated_at,
                ..Default::default()
            },
        )
        .unwrap();
        let row = &s.load_all_notes().unwrap()[0];
        assert_eq!(row.content, "B", "local edit survives a stale server pull");
        assert!(row.dirty, "and stays dirty to push next cycle");
    }

    #[test]
    fn upsert_from_server_overwrites_and_is_clean() {
        let mut s = mem();
        s.sync_enabled = true;
        s.save_note(&Note {
            id: "n1".into(),
            content: "local".into(),
            updated_at: 1,
            ..Default::default()
        })
        .unwrap();
        let local_ts = s.load_dirty_notes().unwrap()[0].updated_at;
        // Server version is newer → wins under LWW, and the row becomes clean.
        upsert_note_from_server_conn(
            &s.conn,
            &Note {
                id: "n1".into(),
                content: "server".into(),
                updated_at: local_ts + 1000,
                ..Default::default()
            },
        )
        .unwrap();
        let all = s.load_all_notes().unwrap();
        assert_eq!(all[0].content, "server");
        assert!(!all[0].dirty);
    }
}
