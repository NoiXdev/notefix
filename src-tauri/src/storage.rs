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
    /// Ciphertext flag (schema v12). `true` means `content` is opaque
    /// ciphertext, not HTML — see `note_protected`/`set_note_protected` and
    /// `sync::note_to_wire`, which carries this flag over the wire alongside
    /// the (always-verbatim) content.
    #[serde(default)]
    pub protected: bool,
    /// Whether the last wire payload carried an explicit `protected`. A pull
    /// from a server that doesn't know the flag must not unprotect a note
    /// (see `upsert_note_from_server_conn`). Local-only; never on the wire.
    #[serde(default, skip_serializing)]
    pub protected_known: bool,
    /// Plaintext title (schema v13), derived from the first line of the
    /// note's content via `note_preview`. Unlike `content`, this is NEVER
    /// encrypted — even for a protected note — so the note stays findable in
    /// the list while its body is sealed. Set via `Store::set_title`,
    /// captured from plaintext before any encryption happens (see
    /// `commands::notes_save` / `commands::encrypt_note_in_place`). Syncs as
    /// plain metadata, like `folder_id`.
    #[serde(default)]
    pub title: String,
    /// "Hide from MCP" opt-out (schema v14). LOCAL only — see
    /// `Store::is_effectively_mcp_hidden`/`set_note_mcp_hidden` and
    /// `sync::note_to_wire`, which deliberately never carries it over the
    /// wire (a device's local hide preference isn't shared data).
    #[serde(default)]
    pub mcp_hidden: bool,
    /// Which workspace-vault key generation sealed this note's ciphertext
    /// (schema v15), or `None` if it's never been sealed / predates
    /// generation tracking. Drives the lazy re-seal work list — see
    /// `Store::notes_with_key_gen_below`.
    #[serde(default)]
    pub key_gen: Option<u32>,
}

pub struct Store {
    pub conn: Connection,
    pub sync_enabled: bool,
}

// `protected` is appended at index 11, `title` at index 12, `mcp_hidden` at
// index 13. `row_to_note` reads all three into `Note` (`mcp_hidden` is never
// forwarded to the sync wire mapping in `sync.rs`, unlike `protected`/
// `title`), and `row_to_meta` reads the same indices: `protected` to blank
// the preview/task counts for ciphertext rows, `title` and `mcp_hidden`
// passed through unchanged. Every query built from `COLS` implicitly carries
// all three — including `search_notes`, which also calls `row_to_meta`.
// `key_gen` is appended at index 14 (schema v15) — read by `row_to_note`
// only; `row_to_meta` doesn't need it and stops at index 13.
const COLS: &str = "id, content, updated_at, pinned, archived, color, due_at, folder_id, position, deleted_at, dirty, protected, title, mcp_hidden, key_gen";

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
        protected: r.get(11)?,
        // Not a stored column — only meaningful for a `Note` freshly built
        // from a sync pull (see `sync::note_from_wire`). A row loaded from
        // local storage has no "did the wire say so" to report.
        protected_known: false,
        title: r.get(12)?,
        mcp_hidden: r.get(13)?,
        key_gen: r.get(14)?,
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
    pub protected: bool,
    /// Plaintext title — see `Note::title`. NOT blanked for a protected note
    /// (unlike `preview`), so the list stays findable while the body is
    /// sealed.
    pub title: String,
    /// "Hide from MCP" flag — see `Note::mcp_hidden`. Exposed here so the
    /// frontend's context menu can show the current state.
    pub mcp_hidden: bool,
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
    let protected: bool = r.get(11)?;
    // Ciphertext never gets run through the preview/task-count heuristics —
    // that would extract garbage (or, in principle, leak a plaintext-looking
    // fragment by coincidence). Protected rows report a blank preview and
    // zero task counts instead. `title`, however, is plaintext metadata even
    // for a protected note (see `Note::title`) — it's read below unchanged,
    // regardless of `protected`.
    let (preview, tasks_done, tasks_total) = if protected {
        (String::new(), 0, 0)
    } else {
        let (tasks_done, tasks_total) = task_counts(&content);
        (note_preview(&content), tasks_done, tasks_total)
    };
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
        preview,
        tasks_done,
        tasks_total,
        protected,
        title: r.get(12)?,
        mcp_hidden: r.get(13)?,
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
    ///
    /// `exclude_protected` drops protected (ciphertext) rows from the candidate
    /// set entirely — while the vault is locked, `content` is ciphertext, so a
    /// plaintext `LIKE`-style scan would both miss real matches and could match
    /// on base64 noise. Callers pass `!vault.is_unlocked()`.
    pub fn search_notes(
        &self,
        query: &str,
        limit: usize,
        exclude_protected: bool,
    ) -> rusqlite::Result<Vec<SearchHit>> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Ok(vec![]);
        }
        let protected_clause = if exclude_protected {
            " AND protected = 0"
        } else {
            ""
        };
        let sql = format!(
            "SELECT {COLS} FROM notes WHERE deleted_at IS NULL AND archived = 0{protected_clause} ORDER BY pinned DESC, position ASC"
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

    /// Mark a note dirty and bump its `updated_at`, but only while sync is
    /// enabled — mirroring `set_folder`/`set_pinned`/`set_archived`, which only
    /// touch `dirty`/`updated_at` in the sync context.
    ///
    /// Used by the protect/lock transitions, whose content write goes through
    /// `set_content_silent` (deliberately no bump). Without this the freshly
    /// encrypted ciphertext row would stay `dirty = 0` and never be pushed, so
    /// the server would keep the pre-protection plaintext; and with `updated_at`
    /// unchanged a later resync could clobber the local ciphertext back to that
    /// plaintext under the last-writer-wins guard.
    pub fn mark_note_dirty_if_syncing(&self, id: &str) -> rusqlite::Result<()> {
        if self.sync_enabled {
            self.conn.execute(
                "UPDATE notes SET updated_at = ?2, dirty = 1 WHERE id = ?1",
                (id, now_ms()),
            )?;
        }
        Ok(())
    }

    pub fn save_note(&self, note: &Note) -> rusqlite::Result<()> {
        let (updated_at, dirty) = if self.sync_enabled {
            (now_ms(), 1)
        } else {
            (note.updated_at, 0)
        };
        self.conn.execute(
            "INSERT INTO notes (id, content, updated_at, pinned, archived, color, due_at, folder_id, position, dirty, key_gen)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at, dirty = excluded.dirty, key_gen = excluded.key_gen",
            (&note.id, &note.content, updated_at, note.pinned, note.archived, &note.color, note.due_at, &note.folder_id, note.position, dirty, note.key_gen),
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

    // Protected-notes vault plumbing (schema v12). `vault_record`/
    // `set_vault_record` are wired in by Task 5's vault commands and
    // `note_protected`/`set_note_protected`/`set_folder_locked`/
    // `note_ids_in_subtree`/`is_effectively_protected` by Task 6's
    // encrypt/decrypt + protect/lock commands (which walk the folder-lock
    // chain via raw SQL in `commands::has_locked_ancestor_folder` rather than
    // through `folder_locked`, so that one — like `clear_vault_record`,
    // reserved for a future vault-reset feature — stays genuinely unused and
    // keeps `#[allow(dead_code)]` per the Task 1/2 precedent in `vault/`.

    /// The stored vault record (opaque JSON blob managed by the crypto layer),
    /// or `None` if no vault has been set up yet. An empty string — the
    /// placeholder `set_vault_entries` inserts when caching entries before
    /// any record exists — also reads back as `None`, not a real record.
    pub fn vault_record(&self) -> rusqlite::Result<Option<String>> {
        use rusqlite::OptionalExtension;
        let rec: Option<String> = self
            .conn
            .query_row("SELECT record FROM vault WHERE id = 1", [], |r| r.get(0))
            .optional()?;
        Ok(rec.filter(|r| !r.is_empty()))
    }

    /// Create or overwrite the single vault record row.
    pub fn set_vault_record(&self, json: &str) -> rusqlite::Result<()> {
        let now = now_ms();
        self.conn.execute(
            "INSERT INTO vault (id, record, created_at, updated_at) VALUES (1, ?1, ?2, ?2)
             ON CONFLICT(id) DO UPDATE SET record = excluded.record, updated_at = excluded.updated_at",
            (json, now),
        )?;
        Ok(())
    }

    /// Remove the vault record — used when a `vault_setup` upload is
    /// rejected by the workspace and the just-written local record has to be
    /// rolled back.
    pub fn clear_vault_record(&self) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM vault WHERE id = 1", [])?;
        Ok(())
    }

    /// The cached JSON of the caller's own wrapped entries from the server
    /// (schema v15) — `{"mine":[...],"recovery":[...]}`, camelCase, matching
    /// the wire shape. `None` if nothing has been cached yet.
    pub fn vault_entries(&self) -> rusqlite::Result<Option<String>> {
        use rusqlite::OptionalExtension;
        self.conn
            .query_row("SELECT entries FROM vault WHERE id = 1", [], |r| r.get(0))
            .optional()
            .map(Option::flatten)
    }

    /// Create or overwrite the cached vault entries JSON. Like
    /// `set_vault_record`, this upserts the single `id = 1` row — if no
    /// vault record exists yet, `record` is seeded with `''` as a
    /// placeholder (the column is `NOT NULL`); `vault_record()` treats an
    /// empty string as "no record" so this doesn't fabricate one.
    pub fn set_vault_entries(&self, json: &str) -> rusqlite::Result<()> {
        let now = now_ms();
        self.conn.execute(
            "INSERT INTO vault (id, record, entries, created_at, updated_at) VALUES (1, '', ?1, ?2, ?2)
             ON CONFLICT(id) DO UPDATE SET entries = excluded.entries, updated_at = excluded.updated_at",
            (json, now),
        )?;
        Ok(())
    }

    pub fn note_protected(&self, id: &str) -> rusqlite::Result<bool> {
        self.conn
            .query_row("SELECT protected FROM notes WHERE id = ?1", [id], |r| {
                r.get(0)
            })
    }

    pub fn set_note_protected(&self, id: &str, v: bool) -> rusqlite::Result<()> {
        self.conn
            .execute("UPDATE notes SET protected = ?2 WHERE id = ?1", (id, v))?;
        Ok(())
    }

    /// Set the note's plaintext title (schema v13). Always plaintext — callers
    /// must derive it from plaintext content BEFORE any encryption, never from
    /// ciphertext. See `commands::notes_save` / `commands::encrypt_note_in_place`.
    pub fn set_title(&self, id: &str, title: &str) -> rusqlite::Result<()> {
        self.conn
            .execute("UPDATE notes SET title = ?2 WHERE id = ?1", (id, title))?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn folder_locked(&self, id: &str) -> rusqlite::Result<bool> {
        self.conn
            .query_row("SELECT locked FROM folders WHERE id = ?1", [id], |r| {
                r.get(0)
            })
    }

    pub fn set_folder_locked(&self, id: &str, v: bool) -> rusqlite::Result<()> {
        self.conn
            .execute("UPDATE folders SET locked = ?2 WHERE id = ?1", (id, v))?;
        // The `locked` flag is folder metadata that must reach the server just
        // like name/icon/etc. — mark the row dirty + bump `updated_at` when
        // syncing so the transition propagates (same convention as
        // `set_pinned`/`set_archived`, both directions).
        if self.sync_enabled {
            self.conn.execute(
                "UPDATE folders SET updated_at = ?2, dirty = 1 WHERE id = ?1",
                (id, now_ms()),
            )?;
        }
        Ok(())
    }

    /// True if `folder_id` — or any of its ancestors — is `locked`. `None`
    /// (root) is never locked. Cycle-safe via a visited set, matching
    /// `is_effectively_protected`'s walk. Shared by the MCP write-guard and,
    /// via `commands::folder_chain_has_lock`, the move/reorder encrypt policy.
    pub fn folder_chain_has_lock(&self, folder_id: Option<&str>) -> rusqlite::Result<bool> {
        use rusqlite::OptionalExtension;
        let mut folder_id: Option<String> = folder_id.map(str::to_string);
        let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
        while let Some(fid) = folder_id {
            if !visited.insert(fid.clone()) {
                break; // cycle detected — stop instead of looping forever
            }
            let folder: Option<(bool, Option<String>)> = self
                .conn
                .query_row(
                    "SELECT locked, parent_id FROM folders WHERE id = ?1",
                    [&fid],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let Some((locked, parent_id)) = folder else {
                break;
            };
            if locked {
                return Ok(true);
            }
            folder_id = parent_id;
        }
        Ok(false)
    }

    /// Ids of notes directly in `folder_id`, plus notes in any of its descendant
    /// folders (recursive).
    pub fn note_ids_in_subtree(&self, folder_id: &str) -> rusqlite::Result<Vec<String>> {
        let mut folder_ids = crate::folders::descendants(&self.conn, folder_id)?;
        folder_ids.push(folder_id.to_string());
        let placeholders = folder_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!("SELECT id FROM notes WHERE folder_id IN ({placeholders})");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(folder_ids.iter()), |r| r.get(0))?;
        rows.collect()
    }

    /// True if the note is individually `protected`, or lives (directly or
    /// nested) inside a `locked` folder.
    ///
    /// Tracks visited folder ids so a cyclic `parent_id` graph — which local
    /// writes never produce, but an unchecked sync pull could via
    /// `folders::upsert_folder_from_server` — terminates the walk instead of
    /// looping forever: re-visiting an id ends the chain rather than erroring.
    pub fn is_effectively_protected(&self, note_id: &str) -> rusqlite::Result<bool> {
        use rusqlite::OptionalExtension;
        let row: Option<(bool, Option<String>)> = self
            .conn
            .query_row(
                "SELECT protected, folder_id FROM notes WHERE id = ?1",
                [note_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((protected, mut folder_id)) = row else {
            return Ok(false);
        };
        if protected {
            return Ok(true);
        }
        let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
        while let Some(fid) = folder_id {
            if !visited.insert(fid.clone()) {
                break; // cycle detected — stop instead of looping forever
            }
            let folder: Option<(bool, Option<String>)> = self
                .conn
                .query_row(
                    "SELECT locked, parent_id FROM folders WHERE id = ?1",
                    [&fid],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let Some((locked, parent_id)) = folder else {
                break;
            };
            if locked {
                return Ok(true);
            }
            folder_id = parent_id;
        }
        Ok(false)
    }

    // "Hide from MCP" plumbing (schema v14) — a LOCAL-only opt-out,
    // independent of the protected-notes vault above. Mirrors
    // `note_protected`/`set_note_protected`/`folder_locked`/
    // `set_folder_locked`/`folder_chain_has_lock`/`is_effectively_protected`
    // field-for-field, but for `mcp_hidden` instead of `protected`/`locked`.
    // Consumed by the MCP surface (`mcp::NoteStore::is_effectively_mcp_hidden`
    // / `folder_chain_has_mcp_hidden`) and the `note_set_mcp_hidden` /
    // `folder_set_mcp_hidden` Tauri commands.

    #[allow(dead_code)] // API symmetry with `note_protected`; not read elsewhere yet.
    pub fn note_mcp_hidden(&self, id: &str) -> rusqlite::Result<bool> {
        self.conn
            .query_row("SELECT mcp_hidden FROM notes WHERE id = ?1", [id], |r| {
                r.get(0)
            })
    }

    pub fn set_note_mcp_hidden(&self, id: &str, v: bool) -> rusqlite::Result<()> {
        self.conn
            .execute("UPDATE notes SET mcp_hidden = ?2 WHERE id = ?1", (id, v))?;
        Ok(())
    }

    #[allow(dead_code)] // API symmetry with `folder_locked`; not read elsewhere yet.
    pub fn folder_mcp_hidden(&self, id: &str) -> rusqlite::Result<bool> {
        self.conn
            .query_row("SELECT mcp_hidden FROM folders WHERE id = ?1", [id], |r| {
                r.get(0)
            })
    }

    pub fn set_folder_mcp_hidden(&self, id: &str, v: bool) -> rusqlite::Result<()> {
        self.conn
            .execute("UPDATE folders SET mcp_hidden = ?2 WHERE id = ?1", (id, v))?;
        Ok(())
    }

    /// True if `folder_id` — or any of its ancestors — has `mcp_hidden` set.
    /// Cycle-safe via a visited set, mirroring `folder_chain_has_lock`.
    pub fn folder_chain_has_mcp_hidden(&self, folder_id: Option<&str>) -> rusqlite::Result<bool> {
        use rusqlite::OptionalExtension;
        let mut folder_id: Option<String> = folder_id.map(str::to_string);
        let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
        while let Some(fid) = folder_id {
            if !visited.insert(fid.clone()) {
                break; // cycle detected — stop instead of looping forever
            }
            let folder: Option<(bool, Option<String>)> = self
                .conn
                .query_row(
                    "SELECT mcp_hidden, parent_id FROM folders WHERE id = ?1",
                    [&fid],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let Some((hidden, parent_id)) = folder else {
                break;
            };
            if hidden {
                return Ok(true);
            }
            folder_id = parent_id;
        }
        Ok(false)
    }

    /// True if the note itself has `mcp_hidden` set, or lives (directly or
    /// nested) inside an `mcp_hidden` folder. Cycle-safe, mirroring
    /// `is_effectively_protected`. This is independent of — and trumps —
    /// `is_effectively_protected`: the MCP surface must treat an
    /// effectively-hidden note as absent whether or not it's also protected.
    pub fn is_effectively_mcp_hidden(&self, note_id: &str) -> rusqlite::Result<bool> {
        use rusqlite::OptionalExtension;
        let row: Option<(bool, Option<String>)> = self
            .conn
            .query_row(
                "SELECT mcp_hidden, folder_id FROM notes WHERE id = ?1",
                [note_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((hidden, folder_id)) = row else {
            return Ok(false);
        };
        if hidden {
            return Ok(true);
        }
        self.folder_chain_has_mcp_hidden(folder_id.as_deref())
    }

    // Workspace vault keys (schema v15) — which key generation sealed a
    // protected note's ciphertext, feeding the lazy re-seal work list after
    // a key rotation. See `Note::key_gen`.

    pub fn note_key_gen(&self, id: &str) -> rusqlite::Result<Option<u32>> {
        self.conn
            .query_row("SELECT key_gen FROM notes WHERE id = ?1", [id], |r| {
                r.get(0)
            })
    }

    pub fn set_note_key_gen(&self, id: &str, gen: Option<u32>) -> rusqlite::Result<()> {
        self.conn
            .execute("UPDATE notes SET key_gen = ?2 WHERE id = ?1", (id, gen))?;
        Ok(())
    }

    /// Protected notes sealed under an older generation than `gen` (or an
    /// unknown one), oldest first — the lazy re-seal work list.
    #[allow(dead_code)] // wired into vault/rotation commands by a later task
    pub fn notes_with_key_gen_below(
        &self,
        gen: u32,
        limit: usize,
    ) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT id FROM notes WHERE protected = 1 AND deleted_at IS NULL
               AND (key_gen IS NULL OR key_gen < ?1) ORDER BY updated_at ASC LIMIT ?2",
        )?;
        let rows = stmt.query_map((gen, limit as i64), |r| r.get::<_, String>(0))?;
        rows.collect()
    }
}

/// Upsert a server note against an arbitrary connection (used inside a tx).
/// Last-write-wins: an existing row is overwritten only when the incoming
/// server version is newer-or-equal (`updated_at`), so a local edit made during
/// the sync window (which has a strictly greater `updated_at`) is preserved and
/// pushed on the next cycle.
pub fn upsert_note_from_server_conn(conn: &rusqlite::Connection, n: &Note) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO notes (id, content, updated_at, pinned, archived, color, due_at, folder_id, position, deleted_at, dirty, protected, title, key_gen)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, ?11, ?12, ?14)
         ON CONFLICT(id) DO UPDATE SET
            content=excluded.content, updated_at=excluded.updated_at, pinned=excluded.pinned,
            archived=excluded.archived, color=excluded.color, due_at=excluded.due_at,
            folder_id=excluded.folder_id, position=excluded.position, deleted_at=excluded.deleted_at, dirty=0,
            protected = CASE WHEN ?13 THEN excluded.protected ELSE notes.protected END, title=excluded.title,
            -- A pull from a server that doesn't know the vault flags yet
            -- (protected_known = false) must not null out a locally known
            -- key generation either — same guard as `protected` above.
            key_gen = CASE WHEN ?13 THEN excluded.key_gen ELSE notes.key_gen END
         WHERE excluded.updated_at >= notes.updated_at",
        (
            &n.id,
            &n.content,
            n.updated_at,
            n.pinned,
            n.archived,
            &n.color,
            n.due_at,
            &n.folder_id,
            n.position,
            n.deleted_at,
            n.protected,
            &n.title,
            n.protected_known,
            n.key_gen,
        ),
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
            protected: false,
            protected_known: false,
            title: String::new(),
            mcp_hidden: false,
            key_gen: None,
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
        let hits = s.search_notes("apple", 50, false).unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].note.id, "title"); // title/preview hit ranks first
        assert!(hits[1].snippet.to_lowercase().contains("apple"));
        assert!(s.search_notes("", 50, false).unwrap().is_empty());
        assert!(s.search_notes("nomatch", 50, false).unwrap().is_empty());
    }

    #[test]
    fn search_notes_can_exclude_protected() {
        let s = store();
        s.save_note(&note("plain", "<p>needle plain</p>", 10))
            .unwrap();
        s.save_note(&note("secret", "<p>needle secret</p>", 20))
            .unwrap();
        s.set_note_protected("secret", true).unwrap();

        // Locked (exclude_protected = true): only the plaintext note matches.
        let hits = s.search_notes("needle", 50, true).unwrap();
        let ids: Vec<_> = hits.iter().map(|h| h.note.id.clone()).collect();
        assert!(ids.contains(&"plain".to_string()));
        assert!(!ids.contains(&"secret".to_string()));

        // Unlocked (exclude_protected = false): both notes match.
        let hits = s.search_notes("needle", 50, false).unwrap();
        let ids: Vec<_> = hits.iter().map(|h| h.note.id.clone()).collect();
        assert!(ids.contains(&"plain".to_string()));
        assert!(ids.contains(&"secret".to_string()));
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
    fn mark_note_dirty_if_syncing_respects_sync_flag() {
        // I1 primitive: the protect/lock transitions bump `dirty`/`updated_at`
        // through this helper (their content write is `set_content_silent`,
        // which deliberately doesn't). It must no-op with sync off and re-dirty
        // + bump with sync on.
        let mut s = mem();
        s.save_note(&note("n1", "<p>x</p>", 5)).unwrap(); // sync off -> clean
        assert!(!s.load_all_notes().unwrap()[0].dirty);

        s.mark_note_dirty_if_syncing("n1").unwrap(); // sync off -> no-op
        let n = &s.load_all_notes().unwrap()[0];
        assert!(!n.dirty);
        assert_eq!(n.updated_at, 5);

        s.sync_enabled = true;
        s.mark_note_dirty_if_syncing("n1").unwrap(); // sync on -> dirty + bump
        let n = &s.load_all_notes().unwrap()[0];
        assert!(n.dirty);
        assert!(n.updated_at > 5);
    }

    #[test]
    fn set_folder_locked_marks_folder_dirty_when_syncing() {
        // I1: the folder `locked` flip is metadata that must reach the server,
        // else another device keeps the folder unlocked. No-op with sync off,
        // dirties the row (either direction) with sync on.
        let mut s = mem();
        crate::folders::create_folder(&s.conn, "f1", "F", None).unwrap();

        s.set_folder_locked("f1", true).unwrap(); // sync off
        assert!(crate::folders::load_dirty_folders(&s.conn)
            .unwrap()
            .is_empty());
        assert!(crate::folders::load_folders(&s.conn).unwrap()[0].locked);

        s.sync_enabled = true;
        s.set_folder_locked("f1", false).unwrap(); // sync on
        let dirty = crate::folders::load_dirty_folders(&s.conn).unwrap();
        assert_eq!(dirty.len(), 1);
        assert_eq!(dirty[0].id, "f1");
        assert!(!dirty[0].locked);
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

    #[test]
    fn pull_without_protected_keeps_the_local_flag() {
        let s = store();
        s.save_note(&note("a", "cipher==", 1000)).unwrap();
        s.set_note_protected("a", true).unwrap();
        let mut pulled = note("a", "cipher==", 2000); // newer, but the server didn't say
        pulled.protected = false;
        pulled.protected_known = false;
        upsert_note_from_server_conn(&s.conn, &pulled).unwrap();
        assert!(s.note_protected("a").unwrap(), "unknown must not unprotect");
        let mut explicit = note("a", "<p>plain</p>", 3000);
        explicit.protected = false;
        explicit.protected_known = true;
        upsert_note_from_server_conn(&s.conn, &explicit).unwrap();
        assert!(!s.note_protected("a").unwrap(), "explicit false unprotects");
    }

    #[test]
    fn note_protected_defaults_false_and_is_settable() {
        let s = store();
        s.save_note(&note("a", "<p>hi</p>", 1000)).unwrap();
        assert!(!s.note_protected("a").unwrap());
        s.set_note_protected("a", true).unwrap();
        assert!(s.note_protected("a").unwrap());
        s.set_note_protected("a", false).unwrap();
        assert!(!s.note_protected("a").unwrap());
    }

    #[test]
    fn folder_locked_defaults_false_and_is_settable() {
        let s = store();
        crate::folders::create_folder(&s.conn, "f", "Secret", None).unwrap();
        assert!(!s.folder_locked("f").unwrap());
        s.set_folder_locked("f", true).unwrap();
        assert!(s.folder_locked("f").unwrap());
    }

    #[test]
    fn effective_protection_via_own_flag() {
        let s = store();
        s.save_note(&note("a", "<p>hi</p>", 1000)).unwrap();
        assert!(!s.is_effectively_protected("a").unwrap());
        s.set_note_protected("a", true).unwrap();
        assert!(s.is_effectively_protected("a").unwrap());
    }

    #[test]
    fn effective_protection_via_locked_folder() {
        let s = store();
        crate::folders::create_folder(&s.conn, "f", "secret", None).unwrap();
        s.save_note(&Note {
            folder_id: Some("f".into()),
            ..note("n", "<p>x</p>", 1000)
        })
        .unwrap();
        assert!(!s.is_effectively_protected("n").unwrap());
        s.set_folder_locked("f", true).unwrap();
        assert!(s.is_effectively_protected("n").unwrap());
    }

    #[test]
    fn effective_protection_via_grandparent_locked_folder() {
        let s = store();
        crate::folders::create_folder(&s.conn, "parent", "Parent", None).unwrap();
        crate::folders::create_folder(&s.conn, "child", "Child", Some("parent")).unwrap();
        s.save_note(&Note {
            folder_id: Some("child".into()),
            ..note("n", "<p>x</p>", 1000)
        })
        .unwrap();
        assert!(!s.is_effectively_protected("n").unwrap());
        s.set_folder_locked("parent", true).unwrap();
        assert!(
            s.is_effectively_protected("n").unwrap(),
            "a locked ancestor two levels up still protects the note"
        );
    }

    #[test]
    fn note_ids_in_subtree_includes_folder_and_descendants_only() {
        let s = store();
        crate::folders::create_folder(&s.conn, "parent", "Parent", None).unwrap();
        crate::folders::create_folder(&s.conn, "child", "Child", Some("parent")).unwrap();
        crate::folders::create_folder(&s.conn, "other", "Other", None).unwrap();
        s.save_note(&Note {
            folder_id: Some("parent".into()),
            ..note("in-parent", "<p>a</p>", 1)
        })
        .unwrap();
        s.save_note(&Note {
            folder_id: Some("child".into()),
            ..note("in-child", "<p>b</p>", 2)
        })
        .unwrap();
        s.save_note(&Note {
            folder_id: Some("other".into()),
            ..note("in-other", "<p>c</p>", 3)
        })
        .unwrap();
        s.save_note(&note("in-root", "<p>d</p>", 4)).unwrap();

        let mut ids = s.note_ids_in_subtree("parent").unwrap();
        ids.sort();
        assert_eq!(ids, vec!["in-child".to_string(), "in-parent".to_string()]);
    }

    /// Guarded write paths (`create_folder`/`move_folder`) can't produce a
    /// folder cycle, but an unchecked sync pull (`upsert_folder_from_server`)
    /// writes `parent_id` straight from wire data with no cycle check. These
    /// two tests insert a cycle directly via raw SQL — bypassing the guarded
    /// paths, the way a bad/malicious sync peer effectively could — and
    /// assert the walks return promptly instead of hanging. Pre-fix, both
    /// would loop forever on this graph; this is a termination check, not a
    /// normal RED/GREEN pair.
    #[test]
    fn is_effectively_protected_terminates_on_cyclic_folder_graph() {
        let s = store();
        crate::folders::create_folder(&s.conn, "a", "A", None).unwrap();
        crate::folders::create_folder(&s.conn, "b", "B", Some("a")).unwrap();
        // Point "a" (b's parent) back at "b", closing the loop a -> b -> a.
        s.conn
            .execute("UPDATE folders SET parent_id = 'b' WHERE id = 'a'", [])
            .unwrap();
        s.save_note(&Note {
            folder_id: Some("b".into()),
            ..note("n", "<p>x</p>", 1000)
        })
        .unwrap();
        assert!(!s.is_effectively_protected("n").unwrap());
        // Also confirm a real lock is still found despite the cycle.
        s.set_folder_locked("a", true).unwrap();
        assert!(s.is_effectively_protected("n").unwrap());
    }

    #[test]
    fn note_ids_in_subtree_terminates_on_cyclic_folder_graph() {
        let s = store();
        crate::folders::create_folder(&s.conn, "a", "A", None).unwrap();
        crate::folders::create_folder(&s.conn, "b", "B", Some("a")).unwrap();
        // Point "a" (b's parent) back at "b", closing the loop a -> b -> a.
        s.conn
            .execute("UPDATE folders SET parent_id = 'b' WHERE id = 'a'", [])
            .unwrap();
        s.save_note(&Note {
            folder_id: Some("a".into()),
            ..note("in-a", "<p>x</p>", 1)
        })
        .unwrap();
        s.save_note(&Note {
            folder_id: Some("b".into()),
            ..note("in-b", "<p>y</p>", 2)
        })
        .unwrap();
        let mut ids = s.note_ids_in_subtree("a").unwrap();
        ids.sort();
        assert_eq!(ids, vec!["in-a".to_string(), "in-b".to_string()]);
    }

    // ---- "Hide from MCP" (schema v14) — mirrors the protected/locked tests
    // above, but for `mcp_hidden` instead of `protected`/`locked`. ----

    #[test]
    fn note_mcp_hidden_defaults_false_and_is_settable() {
        let s = store();
        s.save_note(&note("a", "<p>hi</p>", 1000)).unwrap();
        assert!(!s.note_mcp_hidden("a").unwrap());
        s.set_note_mcp_hidden("a", true).unwrap();
        assert!(s.note_mcp_hidden("a").unwrap());
        s.set_note_mcp_hidden("a", false).unwrap();
        assert!(!s.note_mcp_hidden("a").unwrap());
    }

    #[test]
    fn folder_mcp_hidden_defaults_false_and_is_settable() {
        let s = store();
        crate::folders::create_folder(&s.conn, "f", "Secret", None).unwrap();
        assert!(!s.folder_mcp_hidden("f").unwrap());
        s.set_folder_mcp_hidden("f", true).unwrap();
        assert!(s.folder_mcp_hidden("f").unwrap());
    }

    #[test]
    fn effective_mcp_hidden_via_own_flag() {
        let s = store();
        s.save_note(&note("a", "<p>hi</p>", 1000)).unwrap();
        assert!(!s.is_effectively_mcp_hidden("a").unwrap());
        s.set_note_mcp_hidden("a", true).unwrap();
        assert!(s.is_effectively_mcp_hidden("a").unwrap());
    }

    #[test]
    fn effective_mcp_hidden_via_ancestor_folder() {
        let s = store();
        crate::folders::create_folder(&s.conn, "parent", "Parent", None).unwrap();
        crate::folders::create_folder(&s.conn, "child", "Child", Some("parent")).unwrap();
        s.save_note(&Note {
            folder_id: Some("child".into()),
            ..note("n", "<p>x</p>", 1000)
        })
        .unwrap();
        assert!(!s.is_effectively_mcp_hidden("n").unwrap());
        s.set_folder_mcp_hidden("parent", true).unwrap();
        assert!(
            s.is_effectively_mcp_hidden("n").unwrap(),
            "an mcp_hidden ancestor two levels up still hides the note"
        );
    }

    #[test]
    fn effective_mcp_hidden_is_independent_of_protection() {
        // A note can be effectively-hidden without being protected at all —
        // the two flags are orthogonal (see the doc comment on
        // `is_effectively_mcp_hidden`).
        let s = store();
        s.save_note(&note("a", "<p>hi</p>", 1000)).unwrap();
        s.set_note_mcp_hidden("a", true).unwrap();
        assert!(s.is_effectively_mcp_hidden("a").unwrap());
        assert!(!s.is_effectively_protected("a").unwrap());
    }

    #[test]
    fn is_effectively_mcp_hidden_terminates_on_cyclic_folder_graph() {
        let s = store();
        crate::folders::create_folder(&s.conn, "a", "A", None).unwrap();
        crate::folders::create_folder(&s.conn, "b", "B", Some("a")).unwrap();
        s.conn
            .execute("UPDATE folders SET parent_id = 'b' WHERE id = 'a'", [])
            .unwrap();
        s.save_note(&Note {
            folder_id: Some("b".into()),
            ..note("n", "<p>x</p>", 1000)
        })
        .unwrap();
        assert!(!s.is_effectively_mcp_hidden("n").unwrap());
        s.set_folder_mcp_hidden("a", true).unwrap();
        assert!(s.is_effectively_mcp_hidden("n").unwrap());
    }

    #[test]
    fn load_notes_meta_exposes_mcp_hidden() {
        let s = store();
        s.save_note(&note("a", "<p>hi</p>", 1000)).unwrap();
        s.set_note_mcp_hidden("a", true).unwrap();
        let meta = s.load_notes_meta().unwrap();
        assert!(meta[0].mcp_hidden);
    }

    #[test]
    fn vault_record_roundtrips_and_clears() {
        let s = store();
        assert_eq!(s.vault_record().unwrap(), None);
        s.set_vault_record(r#"{"salt":"abc"}"#).unwrap();
        assert_eq!(
            s.vault_record().unwrap().as_deref(),
            Some(r#"{"salt":"abc"}"#)
        );
        s.set_vault_record(r#"{"salt":"def"}"#).unwrap();
        assert_eq!(
            s.vault_record().unwrap().as_deref(),
            Some(r#"{"salt":"def"}"#)
        );
        s.clear_vault_record().unwrap();
        assert_eq!(s.vault_record().unwrap(), None);
    }

    #[test]
    fn key_gen_helpers_and_lagging_notes() {
        let s = store();
        for (id, gen) in [("a", Some(1)), ("b", Some(2)), ("c", None)] {
            s.save_note(&note(id, "cipher==", 1)).unwrap();
            s.set_note_protected(id, true).unwrap();
            s.set_note_key_gen(id, gen).unwrap();
        }
        s.save_note(&note("plain", "<p>x</p>", 1)).unwrap(); // unprotected, never listed
        assert_eq!(s.note_key_gen("b").unwrap(), Some(2));
        let mut lagging = s.notes_with_key_gen_below(2, 10).unwrap();
        lagging.sort();
        assert_eq!(lagging, vec!["a".to_string(), "c".to_string()]);
    }

    #[test]
    fn vault_entries_roundtrip() {
        let s = store();
        assert_eq!(s.vault_entries().unwrap(), None);
        s.set_vault_entries(r#"{"mine":[],"recovery":[]}"#).unwrap();
        assert_eq!(
            s.vault_entries().unwrap().as_deref(),
            Some(r#"{"mine":[],"recovery":[]}"#)
        );
    }

    #[test]
    fn protected_note_has_blank_preview() {
        let s = store();
        s.save_note(&note(
            "a",
            r#"<p>Title</p><li data-checked="true">x</li>"#,
            1000,
        ))
        .unwrap();
        // Sanity check: an unprotected note gets a real preview + task counts.
        let meta = s.load_notes_meta().unwrap();
        assert_eq!(meta[0].preview, "Title");
        assert_eq!((meta[0].tasks_done, meta[0].tasks_total), (1, 1));
        assert!(!meta[0].protected);

        s.set_note_protected("a", true).unwrap();
        let meta = s.load_notes_meta().unwrap();
        assert_eq!(meta.len(), 1);
        assert_eq!(meta[0].preview, "");
        assert_eq!((meta[0].tasks_done, meta[0].tasks_total), (0, 0));
        assert!(meta[0].protected); // NoteMeta.protected reflects the column
    }

    #[test]
    fn set_title_roundtrips_and_is_not_blanked_when_protected() {
        let s = store();
        s.save_note(&note("a", "<p>original</p>", 1000)).unwrap();
        s.set_title("a", "My Title").unwrap();
        assert_eq!(s.load_notes_meta().unwrap()[0].title, "My Title");

        // Unlike `preview`, `title` stays visible for a protected note — it's
        // metadata, not the secret body.
        s.set_note_protected("a", true).unwrap();
        let meta = &s.load_notes_meta().unwrap()[0];
        assert_eq!(meta.title, "My Title");
        assert_eq!(meta.preview, "");
    }

    #[test]
    fn upsert_from_server_persists_title() {
        let s = mem();
        upsert_note_from_server_conn(
            &s.conn,
            &Note {
                id: "n1".into(),
                content: "<p>x</p>".into(),
                updated_at: 1,
                title: "Server Title".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(s.load_all_notes().unwrap()[0].title, "Server Title");
    }
}
