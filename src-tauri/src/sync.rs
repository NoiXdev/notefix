// src-tauri/src/sync.rs
//
// C1 sync engine. Pure mapping (ms-epoch <-> ISO8601, client model <-> server
// wire) + apply-pulled. Thin network calls + orchestration land in later tasks.

use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::folders::{self, Folder};
use crate::storage::{self, Note, Store};

/// Milliseconds since the Unix epoch -> RFC3339/ISO8601 (UTC), millisecond
/// precision preserved. Empty string on the (practically impossible) overflow.
pub fn ms_to_iso8601(ms: i64) -> String {
    OffsetDateTime::from_unix_timestamp_nanos((ms as i128) * 1_000_000)
        .ok()
        .and_then(|dt| dt.format(&Rfc3339).ok())
        .unwrap_or_default()
}

/// RFC3339/ISO8601 -> milliseconds since the Unix epoch. Accepts a `Z` suffix
/// or a numeric offset (Carbon emits `+00:00`). Returns 0 on parse failure.
pub fn iso8601_to_ms(s: &str) -> i64 {
    OffsetDateTime::parse(s, &Rfc3339)
        .ok()
        .map(|dt| (dt.unix_timestamp_nanos() / 1_000_000) as i64)
        .unwrap_or(0)
}

/// Maps a `Note` to the server wire JSON. `content` is pushed **verbatim** —
/// for a protected note this is already ciphertext (sealed by the vault
/// crypto layer before it ever reaches storage), and nothing in this module
/// decrypts it. `protected` travels alongside it so a receiving device knows
/// the bytes it just pulled are ciphertext and must not be rendered as
/// plaintext. Syncing the flag is safe on its own: it carries no secret, and
/// the content it describes stays unreadable without the vault DEK — which,
/// like the wrapped vault record and the biometric keychain entry, never
/// crosses the wire (a follow-up needs a dedicated vault-record sync entity
/// plus server support before a second device can unlock).
pub fn note_to_wire(n: &Note) -> Value {
    json!({
        "id": n.id,
        "folderId": n.folder_id,
        "content": n.content,
        "pinned": n.pinned,
        "archived": n.archived,
        "color": n.color,
        "dueAt": n.due_at.map(ms_to_iso8601),
        "position": n.position,
        "updatedAt": ms_to_iso8601(n.updated_at),
        "deletedAt": n.deleted_at.map(ms_to_iso8601),
        "protected": n.protected,
        // Plaintext metadata, synced like `folderId` — never encrypted, even
        // for a protected note. See `Note::title`.
        "title": n.title,
    })
}

pub fn note_from_wire(v: &Value) -> Note {
    Note {
        id: v["id"].as_str().unwrap_or_default().to_string(),
        content: v["content"].as_str().unwrap_or_default().to_string(),
        updated_at: v["updatedAt"].as_str().map(iso8601_to_ms).unwrap_or(0),
        pinned: v["pinned"].as_bool().unwrap_or(false),
        archived: v["archived"].as_bool().unwrap_or(false),
        color: v["color"].as_str().unwrap_or_default().to_string(),
        due_at: v["dueAt"].as_str().map(iso8601_to_ms),
        folder_id: v["folderId"].as_str().map(str::to_string),
        position: v["position"].as_i64().unwrap_or(0),
        deleted_at: v["deletedAt"].as_str().map(iso8601_to_ms),
        dirty: false,
        // Missing/unknown-to-server field defaults to unprotected — matches
        // today's behavior for a server that doesn't persist this field yet.
        protected: v["protected"].as_bool().unwrap_or(false),
        title: v["title"].as_str().unwrap_or_default().to_string(),
    }
}

pub fn folder_to_wire(f: &Folder) -> Value {
    json!({
        "id": f.id,
        "parentId": f.parent_id,
        "name": f.name,
        "icon": f.icon,
        "color": f.color,
        "sort": f.sort,
        "position": f.position,
        "updatedAt": ms_to_iso8601(f.updated_at),
        "deletedAt": f.deleted_at.map(ms_to_iso8601),
        "locked": f.locked,
    })
}

pub fn folder_from_wire(v: &Value) -> Folder {
    Folder {
        id: v["id"].as_str().unwrap_or_default().to_string(),
        name: v["name"].as_str().unwrap_or_default().to_string(),
        parent_id: v["parentId"].as_str().map(str::to_string),
        position: v["position"].as_i64().unwrap_or(0),
        icon: v["icon"].as_str().unwrap_or_default().to_string(),
        color: v["color"].as_str().unwrap_or_default().to_string(),
        sort: v["sort"].as_str().unwrap_or_default().to_string(),
        updated_at: v["updatedAt"].as_str().map(iso8601_to_ms).unwrap_or(0),
        deleted_at: v["deletedAt"].as_str().map(iso8601_to_ms),
        dirty: false,
        // `locked` now syncs too — same reasoning as `Note.protected` in
        // `note_to_wire` above: it only labels content that's already
        // ciphertext, so the flag alone carries no secret. Missing/unknown
        // field defaults to unlocked, matching today's behavior for a server
        // that doesn't persist this field yet.
        locked: v["locked"].as_bool().unwrap_or(false),
    }
}

/// Apply a pulled batch to the local cache (server rows win) in one transaction.
pub fn apply_pulled(store: &Store, folders: &[Value], notes: &[Value]) -> rusqlite::Result<()> {
    let tx = store.conn.unchecked_transaction()?;
    for fv in folders {
        folders::upsert_folder_from_server(&tx, &folder_from_wire(fv))?;
    }
    for nv in notes {
        storage::upsert_note_from_server_conn(&tx, &note_from_wire(nv))?;
    }
    tx.commit()
}

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub id: String,
    pub name: String,
    pub role: String,
}

/// Sync failure kinds. `Offline` is retryable (network/timeout/connection/401);
/// `Fatal` is a payload/server error that retrying won't fix.
#[derive(Debug)]
pub enum SyncError {
    Offline(String),
    Fatal(String),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::Offline(m) | SyncError::Fatal(m) => write!(f, "{m}"),
        }
    }
}

fn client() -> Result<reqwest::Client, SyncError> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| SyncError::Fatal(e.to_string()))
}

fn base(server_url: &str) -> String {
    server_url.trim_end_matches('/').to_string()
}

/// GET /api/workspaces — the user's workspaces for the picker.
pub async fn fetch_workspaces(
    server_url: &str,
    token: &str,
) -> Result<Vec<WorkspaceInfo>, SyncError> {
    let url = format!("{}/api/workspaces", base(server_url));
    let resp = client()?
        .get(&url)
        .bearer_auth(token)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| SyncError::Offline(e.to_string()))?;
    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(SyncError::Offline("unauthorized".into()));
    }
    if !resp.status().is_success() {
        return Err(SyncError::Fatal(format!(
            "workspaces HTTP {}",
            resp.status().as_u16()
        )));
    }
    let body: Value = resp
        .json()
        .await
        .map_err(|e| SyncError::Fatal(e.to_string()))?;
    let rows = body["data"].as_array().cloned().unwrap_or_default();
    Ok(rows
        .iter()
        .map(|w| WorkspaceInfo {
            id: w["id"].as_str().unwrap_or_default().to_string(),
            name: w["name"].as_str().unwrap_or_default().to_string(),
            role: w["role"].as_str().unwrap_or_default().to_string(),
        })
        .collect())
}

/// GET …/changes?since= → (cursor, folders, notes) as raw wire values.
pub async fn pull(
    server_url: &str,
    token: &str,
    workspace_id: &str,
    since: i64,
) -> Result<(i64, Vec<Value>, Vec<Value>), SyncError> {
    let url = format!(
        "{}/api/workspaces/{}/changes?since={}",
        base(server_url),
        workspace_id,
        since
    );
    let resp = client()?
        .get(&url)
        .bearer_auth(token)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| SyncError::Offline(e.to_string()))?;
    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(SyncError::Offline("unauthorized".into()));
    }
    if !resp.status().is_success() {
        return Err(SyncError::Fatal(format!(
            "pull HTTP {}",
            resp.status().as_u16()
        )));
    }
    let body: Value = resp
        .json()
        .await
        .map_err(|e| SyncError::Fatal(e.to_string()))?;
    let cursor = body["cursor"].as_i64().unwrap_or(since);
    let folders = body["folders"]["data"]
        .as_array()
        .or(body["folders"].as_array())
        .cloned()
        .unwrap_or_default();
    let notes = body["notes"]["data"]
        .as_array()
        .or(body["notes"].as_array())
        .cloned()
        .unwrap_or_default();
    Ok((cursor, folders, notes))
}

/// POST …/changes with dirty folders+notes; returns the server's new cursor.
pub async fn push(
    server_url: &str,
    token: &str,
    workspace_id: &str,
    folders: Vec<Value>,
    notes: Vec<Value>,
) -> Result<i64, SyncError> {
    let url = format!(
        "{}/api/workspaces/{}/changes",
        base(server_url),
        workspace_id
    );
    let resp = client()?
        .post(&url)
        .bearer_auth(token)
        .header("Accept", "application/json")
        .json(&json!({ "folders": folders, "notes": notes }))
        .send()
        .await
        .map_err(|e| SyncError::Offline(e.to_string()))?;
    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(SyncError::Offline("unauthorized".into()));
    }
    if !resp.status().is_success() {
        return Err(SyncError::Fatal(format!(
            "push HTTP {}",
            resp.status().as_u16()
        )));
    }
    let body: Value = resp
        .json()
        .await
        .map_err(|e| SyncError::Fatal(e.to_string()))?;
    Ok(body["cursor"].as_i64().unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_ms_roundtrip_preserves_milliseconds() {
        let ms = 1_700_000_000_123; // not second-aligned: proves ms precision
        assert_eq!(iso8601_to_ms(&ms_to_iso8601(ms)), ms);
    }

    #[test]
    fn parses_carbon_style_numeric_offset() {
        // Carbon's toIso8601String() emits a +00:00 offset, not Z.
        assert_eq!(
            iso8601_to_ms("2023-11-14T22:13:20+00:00"),
            1_700_000_000_000
        );
    }

    #[test]
    fn note_wire_roundtrip() {
        let n = Note {
            id: "n1".into(),
            content: "<p>x</p>".into(),
            updated_at: 1_700_000_000_000,
            pinned: true,
            archived: false,
            color: "red".into(),
            due_at: Some(1_700_000_001_000),
            folder_id: Some("f1".into()),
            position: 3,
            deleted_at: None,
            dirty: true,
            protected: false,
            title: "My Title".into(),
        };
        let back = note_from_wire(&note_to_wire(&n));
        assert_eq!(back.id, "n1");
        assert_eq!(back.updated_at, n.updated_at);
        assert_eq!(back.due_at, n.due_at);
        assert_eq!(back.folder_id, n.folder_id);
        assert!(!back.dirty); // wire never carries dirty
        assert!(!back.protected);
        assert_eq!(back.title, n.title);
    }

    #[test]
    fn note_from_wire_defaults_title_empty_when_field_missing() {
        // A server that doesn't (yet) persist `title` simply omits it.
        let v = json!({ "id": "n1", "content": "<p>x</p>" });
        assert_eq!(note_from_wire(&v).title, "");
    }

    #[test]
    fn note_wire_roundtrip_preserves_protected_ciphertext() {
        // Opaque, base64-like blob standing in for real vault ciphertext — the
        // point of this test is that sync never inspects or decrypts it.
        let ciphertext = "v1:Zm9vYmFyYmF6cXV1eA==:9f8a7b6c5d4e3f2a1b0c";
        let n = Note {
            id: "n1".into(),
            content: ciphertext.into(),
            updated_at: 1_700_000_000_000,
            protected: true,
            ..Default::default()
        };

        let wire = note_to_wire(&n);
        // The wire payload carries the ciphertext byte-for-byte, plus the flag
        // that tells the receiver it IS ciphertext — never plaintext.
        assert_eq!(wire["content"], ciphertext);
        assert_eq!(wire["protected"], true);

        let back = note_from_wire(&wire);
        assert!(back.protected);
        assert_eq!(back.content, ciphertext);
    }

    #[test]
    fn note_from_wire_defaults_protected_false_when_field_missing() {
        // A server that doesn't (yet) persist `protected` simply omits it —
        // must not be mistaken for an explicit `false` that unprotects a note.
        let v = json!({ "id": "n1", "content": "<p>x</p>" });
        assert!(!note_from_wire(&v).protected);
    }

    #[test]
    fn folder_wire_roundtrip_and_tombstone() {
        let f = Folder {
            id: "f1".into(),
            name: "Work".into(),
            parent_id: None,
            position: 2,
            icon: "star".into(),
            color: "blue".into(),
            sort: "manual".into(),
            updated_at: 1_700_000_000_000,
            deleted_at: Some(1_700_000_005_000),
            dirty: true,
            locked: false,
        };
        let back = folder_from_wire(&folder_to_wire(&f));
        assert_eq!(back.name, "Work");
        assert_eq!(back.deleted_at, f.deleted_at);
        assert!(!back.locked);
    }

    #[test]
    fn folder_wire_roundtrip_preserves_locked() {
        let f = Folder {
            id: "f1".into(),
            name: "Secrets".into(),
            locked: true,
            ..Default::default()
        };
        let wire = folder_to_wire(&f);
        assert_eq!(wire["locked"], true);
        let back = folder_from_wire(&wire);
        assert!(back.locked);
    }

    #[test]
    fn apply_pulled_overwrites_local() {
        let s = Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        s.save_note(&Note {
            id: "n1".into(),
            content: "local".into(),
            updated_at: 1,
            ..Default::default()
        })
        .unwrap();
        let server_note = note_to_wire(&Note {
            id: "n1".into(),
            content: "server".into(),
            updated_at: 1_700_000_000_000,
            ..Default::default()
        });
        apply_pulled(&s, &[], &[server_note]).unwrap();
        assert_eq!(s.load_all_notes().unwrap()[0].content, "server");
    }

    #[test]
    fn apply_pulled_persists_protected_and_locked_to_db() {
        // The wire-mapping tests above only check note_to_wire/note_from_wire
        // in isolation. This exercises the real pull-apply path
        // (apply_pulled -> upsert_note_from_server_conn /
        // upsert_folder_from_server) to confirm the flags actually land in
        // the SQLite columns, not just the in-memory struct.
        let s = Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();

        let ciphertext = "v1:c2VhbGVkLWJ5dGVz:deadbeef";
        let server_note = note_to_wire(&Note {
            id: "n1".into(),
            content: ciphertext.into(),
            updated_at: 1_700_000_000_000,
            protected: true,
            ..Default::default()
        });
        let server_folder = folder_to_wire(&Folder {
            id: "f1".into(),
            name: "Secrets".into(),
            updated_at: 1_700_000_000_000,
            locked: true,
            ..Default::default()
        });

        apply_pulled(&s, &[server_folder], &[server_note]).unwrap();

        let notes = s.load_all_notes().unwrap();
        assert_eq!(notes.len(), 1);
        assert!(notes[0].protected, "protected flag must persist to the DB");
        // Content stays exactly as received — never decrypted on the way in.
        assert_eq!(notes[0].content, ciphertext);

        let folders = folders::load_folders(&s.conn).unwrap();
        assert_eq!(folders.len(), 1);
        assert!(folders[0].locked, "locked flag must persist to the DB");
    }

    #[test]
    fn sync_error_display() {
        assert_eq!(SyncError::Offline("x".into()).to_string(), "x");
        assert_eq!(SyncError::Fatal("y".into()).to_string(), "y");
    }
}
