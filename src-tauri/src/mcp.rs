use crate::folders::Folder;
use crate::storage::Note;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager};

pub trait NoteStore: Send + Sync {
    /// All notes incl. archived and trashed (like `load_all_notes`).
    fn all_notes(&self) -> Result<Vec<Note>, String>;
    fn get_note(&self, id: &str) -> Result<Option<Note>, String>;
    /// Upsert. On an existing row only content/updated_at change (see Global
    /// Constraints) — used for create (insert), append, and content replace.
    fn save(&self, note: &Note) -> Result<(), String>;
    fn set_folder(&self, id: &str, folder_id: Option<&str>) -> Result<(), String>;
    fn set_archived(&self, id: &str, archived: bool) -> Result<(), String>;
    fn trash(&self, id: &str, ts: i64) -> Result<(), String>;
    /// Clear the trash tombstone (does not touch `archived`).
    fn untrash(&self, id: &str) -> Result<(), String>;
    fn list_folders(&self) -> Result<Vec<Folder>, String>;
    /// Create a folder with a fresh id; returns the created `Folder`.
    fn create_folder(&self, name: &str, parent_id: Option<&str>) -> Result<Folder, String>;
    /// True if the note is individually protected or lives inside a locked
    /// folder. The vault is process-local, so the MCP surface must never
    /// expose or mutate such a note: reads exclude it, writes refuse it.
    fn is_effectively_protected(&self, id: &str) -> Result<bool, String>;
    /// True if `folder_id` (or any ancestor) is locked — used to refuse
    /// creating/placing a plaintext note inside a locked subtree.
    fn folder_chain_has_lock(&self, folder_id: Option<&str>) -> Result<bool, String>;
    /// True if the note is individually hidden-from-MCP, or lives inside a
    /// folder marked hidden-from-MCP (or a nested subtree thereof). This is a
    /// LOCAL-only opt-out (schema v14), independent of protection, and trumps
    /// everything: reads exclude it, writes refuse it, whether or not the
    /// note is also protected.
    fn is_effectively_mcp_hidden(&self, id: &str) -> Result<bool, String>;
    /// True if `folder_id` (or any ancestor) is hidden-from-MCP — used to
    /// refuse creating/placing/moving a note into a hidden subtree, mirroring
    /// `folder_chain_has_lock`.
    fn folder_chain_has_mcp_hidden(&self, folder_id: Option<&str>) -> Result<bool, String>;
    /// The live `mcpProtectedAccess` setting ("off"/"read"/"readwrite"),
    /// read fresh on every call so a settings change takes effect without an
    /// MCP server restart.
    fn mcp_protected_access(&self) -> String;
    /// True if the protected-notes vault currently holds an unlocked DEK.
    fn vault_unlocked(&self) -> bool;
    /// Decrypts an effectively-protected note's stored ciphertext `content`
    /// into plaintext HTML. Callers must already have confirmed the vault is
    /// unlocked and the read policy allows it — this only performs the AEAD
    /// open. Any failure (locked vault, corrupt/foreign ciphertext) is an
    /// `Err`; callers must treat that as "note absent", never return the
    /// ciphertext or a partial result.
    fn decrypt_protected(&self, id: &str, ciphertext: &str) -> Result<String, String>;
    /// Overwrites an effectively-protected note's content with
    /// `plaintext_html` and immediately reseals it (ciphertext on disk,
    /// `protected` stays true), atomically under one store-lock acquisition
    /// so no concurrent MCP request can observe the note as plaintext at
    /// rest. Requires the vault to be unlocked.
    fn write_protected(&self, id: &str, plaintext_html: &str) -> Result<(), String>;
    fn now_ms(&self) -> i64;
    fn new_id(&self) -> String;
    fn emit_changed(&self);
}

pub fn html_to_text(html: &str) -> String {
    let nl = regex::Regex::new(r"(?is)</(p|div|h[1-6]|li)>|<br\s*/?>")
        .unwrap()
        .replace_all(html, "\n");
    let stripped = regex::Regex::new(r"(?is)<[^>]+>")
        .unwrap()
        .replace_all(&nl, "");
    stripped
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .trim()
        .to_string()
}

fn ok(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}
fn rpc_err(id: &Value, code: i64, msg: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": msg } })
}

fn tool_defs() -> Value {
    json!([
        { "name": "list_notes", "description": "List notes as JSON. Each item: {id, title, group, contentType, status, updatedAt}. Optional filters: status (active|archived|trashed|all, default active) and groupId.",
          "inputSchema": { "type": "object", "properties": {
            "status": { "type": "string", "enum": ["active", "archived", "trashed", "all"] },
            "groupId": { "type": "string" } } } },
        { "name": "get_note", "description": "Get one note as JSON including its content. Content is returned as Markdown by default (contentType 'markdown'); pass format 'html' or 'text' for other representations.",
          "inputSchema": { "type": "object", "properties": {
            "id": { "type": "string" },
            "format": { "type": "string", "enum": ["markdown", "html", "text"] } }, "required": ["id"] } },
        { "name": "search_notes", "description": "Search notes by text (case-insensitive). Returns JSON summaries with a snippet. Optional status (default active) and groupId filters.",
          "inputSchema": { "type": "object", "properties": {
            "query": { "type": "string" },
            "status": { "type": "string", "enum": ["active", "archived", "trashed", "all"] },
            "groupId": { "type": "string" } }, "required": ["query"] } },
        { "name": "list_groups", "description": "List groups (folders) as JSON: {id, name, parentId, path}. Use these ids/names to target a group when creating or moving notes.",
          "inputSchema": { "type": "object", "properties": {} } },
        { "name": "create_note", "description": "Create a note. 'content' is GitHub-Flavored Markdown by default and is converted to the app's rich format (headings, bold, lists, task lists, tables). Pass format 'text' to override. Optionally place it in a group via groupId or groupName (groupName must match exactly one existing group).",
          "inputSchema": { "type": "object", "properties": {
            "content": { "type": "string" },
            "format": { "type": "string", "enum": ["markdown", "text"] },
            "groupId": { "type": "string" },
            "groupName": { "type": "string" } }, "required": ["content"] } },
        { "name": "append_note", "description": "Append to a note. 'text' is Markdown by default (format 'text' to override) and is converted before appending.",
          "inputSchema": { "type": "object", "properties": {
            "id": { "type": "string" },
            "text": { "type": "string" },
            "format": { "type": "string", "enum": ["markdown", "text"] } }, "required": ["id", "text"] } },
        { "name": "update_note", "description": "Replace a note's whole content (Markdown by default; format 'text' to override) and/or move it to another group via groupId or groupName. Omit 'content' to move only.",
          "inputSchema": { "type": "object", "properties": {
            "id": { "type": "string" },
            "content": { "type": "string" },
            "format": { "type": "string", "enum": ["markdown", "text"] },
            "groupId": { "type": "string" },
            "groupName": { "type": "string" } }, "required": ["id"] } },
        { "name": "create_group", "description": "Create a group (folder). Optionally nest it under an existing group via parentId. Returns the new group {id, name, parentId, path}.",
          "inputSchema": { "type": "object", "properties": {
            "name": { "type": "string" },
            "parentId": { "type": "string" } }, "required": ["name"] } },
        { "name": "archive_note", "description": "Archive a note (moves it out of the active list; reversible with restore_note).",
          "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } },
        { "name": "delete_note", "description": "Move a note to trash (soft-delete; reversible with restore_note).",
          "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } },
        { "name": "restore_note", "description": "Restore a note to active from trash or archive.",
          "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } }
    ])
}

fn status_of(n: &Note) -> &'static str {
    if n.deleted_at.is_some() {
        "trashed"
    } else if n.archived {
        "archived"
    } else {
        "active"
    }
}

fn folder_path(folders: &[Folder], id: &str) -> String {
    let mut names = Vec::new();
    let mut cur = Some(id.to_string());
    let mut guard = 0;
    while let Some(cid) = cur {
        guard += 1;
        if guard > 64 {
            break; // defensive against cycles
        }
        match folders.iter().find(|f| f.id == cid) {
            Some(f) => {
                names.push(f.name.clone());
                cur = f.parent_id.clone();
            }
            None => break,
        }
    }
    names.reverse();
    names.join("/")
}

fn group_json(folders: &[Folder], folder_id: Option<&str>) -> Value {
    match folder_id {
        Some(fid) => match folders.iter().find(|f| f.id == fid) {
            Some(f) => json!({ "id": f.id, "name": f.name, "path": folder_path(folders, fid) }),
            None => Value::Null,
        },
        None => Value::Null,
    }
}

fn resolve_group(
    store: &dyn NoteStore,
    group_id: Option<&str>,
    group_name: Option<&str>,
) -> Result<Option<String>, String> {
    match (group_id, group_name) {
        (Some(_), Some(_)) => Err("specify groupId or groupName, not both".into()),
        (Some(id), None) => {
            let folders = store.list_folders()?;
            if folders.iter().any(|f| f.id == id) {
                Ok(Some(id.to_string()))
            } else {
                Err("group not found".into())
            }
        }
        (None, Some(name)) => {
            let folders = store.list_folders()?;
            let matches: Vec<&Folder> = folders
                .iter()
                .filter(|f| f.name.to_lowercase() == name.to_lowercase())
                .collect();
            match matches.as_slice() {
                [] => Err("group not found".into()),
                [one] => Ok(Some(one.id.clone())),
                many => Err(format!(
                    "ambiguous group name '{}': {}",
                    name,
                    many.iter()
                        .map(|f| f.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            }
        }
        (None, None) => Ok(None),
    }
}

fn note_summary(n: &Note, folders: &[Folder]) -> Value {
    json!({
        "id": n.id,
        "title": crate::mdconv::title_from_html(&n.content),
        "group": group_json(folders, n.folder_id.as_deref()),
        "contentType": "markdown",
        "status": status_of(n),
        "updatedAt": n.updated_at,
    })
}

fn note_full(n: &Note, folders: &[Folder], content: &str, content_type: &str) -> Value {
    let mut v = note_summary(n, folders);
    v["content"] = json!(content);
    v["contentType"] = json!(content_type);
    v["pinned"] = json!(n.pinned);
    v["dueAt"] = json!(n.due_at);
    v
}

/// A short window (~120 chars) of `plain` around the first occurrence of the
/// already-lowercased `q`, with leading/trailing ellipses when clipped.
fn snippet(plain: &str, q: &str) -> String {
    let chars: Vec<char> = plain.chars().collect();
    let lower = plain.to_lowercase();
    let Some(byte_idx) = lower.find(q) else {
        return chars
            .iter()
            .take(120)
            .collect::<String>()
            .trim()
            .to_string();
    };
    // `lower` (from `to_lowercase()`) can have a different char count than
    // the original `chars` (some characters expand when lowercased, e.g.
    // 'İ' -> "i̇"), so a char index computed from `lower`'s bytes must be
    // clamped to `chars.len()` before it's used to slice `chars` — otherwise
    // `start` (or `end`) could exceed `chars.len()` and panic the slice.
    let start = lower[..byte_idx]
        .chars()
        .count()
        .saturating_sub(40)
        .min(chars.len());
    let end = (start + 120).min(chars.len());
    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.push_str(chars[start..end].iter().collect::<String>().trim());
    if end < chars.len() {
        out.push('…');
    }
    out
}

fn to_html(content: &str, format: Option<&str>) -> String {
    match format.unwrap_or("markdown") {
        "text" => crate::mdconv::wrap_plaintext(content),
        // "markdown", None, or any other/unrecognized value (including the
        // now-removed "html" override): a write must never store caller-
        // supplied HTML verbatim, so it always goes through the Markdown
        // converter.
        _ => crate::mdconv::md_to_html(content),
    }
}

/// Resolves a note for a READ tool (list_notes/get_note/search_notes).
/// `Ok(None)` means MCP must treat it as absent:
/// - effectively hidden-from-MCP (trumps everything), or
/// - effectively protected while `mcpProtectedAccess` is "off" or the vault
///   is locked (today's default-safe behavior, unchanged), or
/// - effectively protected, access allowed, vault unlocked, but decrypting
///   its ciphertext failed — never surface ciphertext or a partial result.
///
/// Otherwise returns a clone of `n` whose `content` is guaranteed to be
/// plaintext HTML (decrypted on the fly for an allowed protected note), so
/// callers can feed it straight into `note_summary`/`note_full` unchanged.
fn visible_for_read(store: &dyn NoteStore, n: &Note) -> Result<Option<Note>, String> {
    if store.is_effectively_mcp_hidden(&n.id)? {
        return Ok(None);
    }
    if store.is_effectively_protected(&n.id)? {
        if store.mcp_protected_access() == "off" || !store.vault_unlocked() {
            return Ok(None);
        }
        return Ok(match store.decrypt_protected(&n.id, &n.content) {
            Ok(plaintext) => {
                let mut opened = n.clone();
                opened.content = plaintext;
                Some(opened)
            }
            Err(_) => None,
        });
    }
    Ok(Some(n.clone()))
}

/// Checks whether `id`'s note may be written to via MCP (append_note /
/// update_note). `Ok(true)` = write allowed AND the note is effectively
/// protected — the caller must go through `decrypt_protected` +
/// `write_protected` rather than the plain `save` path. `Ok(false)` = write
/// allowed and the note is plain. `Err` = refuse outright:
/// - effectively hidden-from-MCP (trumps everything, checked first), or
/// - effectively protected but `mcpProtectedAccess` isn't "readwrite", or the
///   vault is locked.
fn writable_protected(store: &dyn NoteStore, id: &str) -> Result<bool, String> {
    if store.is_effectively_mcp_hidden(id)? {
        return Err("note hidden from MCP".into());
    }
    let protected = store.is_effectively_protected(id)?;
    if protected && (store.mcp_protected_access() != "readwrite" || !store.vault_unlocked()) {
        return Err("note is protected".into());
    }
    Ok(protected)
}

fn call_tool(
    name: &str,
    args: &Value,
    store: &dyn NoteStore,
    allow_write: bool,
) -> Result<String, String> {
    let s = |k: &str| args.get(k).and_then(|v| v.as_str()).map(|v| v.to_string());
    match name {
        "list_notes" => {
            let status = s("status").unwrap_or_else(|| "active".into());
            let group = s("groupId");
            let folders = store.list_folders()?;
            let mut items: Vec<Value> = Vec::new();
            for n in store.all_notes()? {
                if status != "all" && status_of(&n) != status {
                    continue;
                }
                if group
                    .as_deref()
                    .is_some_and(|g| n.folder_id.as_deref() != Some(g))
                {
                    continue;
                }
                // Hidden-from-MCP notes never appear; a protected note appears
                // only when `mcpProtectedAccess` allows it (decrypted) — see
                // `visible_for_read`. Default settings behave exactly as
                // before: protected notes stay excluded.
                let Some(n) = visible_for_read(store, &n)? else {
                    continue;
                };
                items.push(note_summary(&n, &folders));
            }
            Ok(json!(items).to_string())
        }
        "get_note" => {
            let id = s("id").unwrap_or_default();
            let n = store.get_note(&id)?.ok_or("note not found")?;
            // Hidden/disallowed-protected notes are invisible to MCP — report
            // them as absent rather than leaking ciphertext/a ciphertext-
            // derived title, or an existence signal.
            let n = visible_for_read(store, &n)?.ok_or("note not found")?;
            let folders = store.list_folders()?;
            let fmt = s("format").unwrap_or_else(|| "markdown".into());
            let content = match fmt.as_str() {
                "html" => n.content.clone(),
                "text" => html_to_text(&n.content),
                _ => crate::mdconv::html_to_md(&n.content),
            };
            let ct = match fmt.as_str() {
                "html" => "html",
                "text" => "text",
                _ => "markdown",
            };
            Ok(note_full(&n, &folders, &content, ct).to_string())
        }
        "search_notes" => {
            let q = s("query").unwrap_or_default().to_lowercase();
            let status = s("status").unwrap_or_else(|| "active".into());
            let group = s("groupId");
            let folders = store.list_folders()?;
            let mut items: Vec<Value> = Vec::new();
            for n in store.all_notes()? {
                if status != "all" && status_of(&n) != status {
                    continue;
                }
                if group
                    .as_deref()
                    .is_some_and(|g| n.folder_id.as_deref() != Some(g))
                {
                    continue;
                }
                // Hidden notes are never scanned; a protected note's ciphertext
                // is only decrypted (and then scanned) when policy allows it —
                // see `visible_for_read`.
                let Some(n) = visible_for_read(store, &n)? else {
                    continue;
                };
                let plain = html_to_text(&n.content);
                if q.is_empty() || !plain.to_lowercase().contains(&q) {
                    continue;
                }
                let mut v = note_summary(&n, &folders);
                v["snippet"] = json!(snippet(&plain, &q));
                items.push(v);
            }
            Ok(json!(items).to_string())
        }
        "list_groups" => {
            let folders = store.list_folders()?;
            let items: Vec<Value> = folders
                .iter()
                .map(|f| {
                    json!({
                        "id": f.id,
                        "name": f.name,
                        "parentId": f.parent_id,
                        "path": folder_path(&folders, &f.id),
                    })
                })
                .collect();
            Ok(json!(items).to_string())
        }
        "create_note" => {
            if !allow_write {
                return Err("writing disabled".into());
            }
            let content = s("content").ok_or("content is required")?;
            let folder_id =
                resolve_group(store, s("groupId").as_deref(), s("groupName").as_deref())?;
            // Refuse to drop a plaintext note into a locked subtree — MCP has no
            // vault key, so it can't encrypt-on-place the way the app does.
            if store.folder_chain_has_lock(folder_id.as_deref())? {
                return Err("folder is locked".into());
            }
            // Same refusal for a hidden-from-MCP subtree — a note created
            // there would be effectively hidden the instant it's placed.
            if store.folder_chain_has_mcp_hidden(folder_id.as_deref())? {
                return Err("folder hidden from MCP".into());
            }
            let note = Note {
                id: store.new_id(),
                content: to_html(&content, s("format").as_deref()),
                updated_at: store.now_ms(),
                folder_id: folder_id.clone(),
                ..Default::default()
            };
            store.save(&note)?;
            store.emit_changed();
            let folders = store.list_folders()?;
            Ok(json!({
                "id": note.id,
                "group": group_json(&folders, folder_id.as_deref()),
                "status": "active",
            })
            .to_string())
        }
        "append_note" => {
            if !allow_write {
                return Err("writing disabled".into());
            }
            let id = s("id").ok_or("id is required")?;
            let text = s("text").ok_or("text is required")?;
            // Hidden-from-MCP notes are never writable, full stop. A protected
            // note's stored content is ciphertext: appending plaintext would
            // both leak it at rest and corrupt the blob, so it's refused
            // UNLESS `mcpProtectedAccess` is "readwrite" and the vault is
            // unlocked — in which case the addition is appended to the
            // decrypted plaintext and the whole thing is resealed atomically,
            // never leaving plaintext on disk. Touches nothing on refusal.
            let protected = writable_protected(store, &id)?;
            let mut note = store.get_note(&id)?.ok_or("note not found")?;
            let addition = to_html(&text, s("format").as_deref());
            if protected {
                let plaintext = store.decrypt_protected(&id, &note.content)?;
                store.write_protected(&id, &format!("{plaintext}{addition}"))?;
            } else {
                note.content.push_str(&addition);
                note.updated_at = store.now_ms();
                store.save(&note)?;
            }
            store.emit_changed();
            Ok(json!({ "id": note.id, "status": status_of(&note) }).to_string())
        }
        "update_note" => {
            if !allow_write {
                return Err("writing disabled".into());
            }
            let id = s("id").ok_or("id is required")?;
            // Resolve the group (when present) BEFORE any mutation, mirroring
            // create_note's ordering: an invalid/ambiguous groupId/groupName
            // must error out before content is saved, so a bad group can
            // never leave the note's content mutated with the error still
            // returned.
            let folder_id = if args.get("groupId").is_some() || args.get("groupName").is_some() {
                Some(resolve_group(
                    store,
                    s("groupId").as_deref(),
                    s("groupName").as_deref(),
                )?)
            } else {
                None
            };
            // Refuse — before any mutation — a hidden-from-MCP note (trumps
            // everything) or a protected note MCP isn't allowed to touch
            // (`mcpProtectedAccess` isn't "readwrite", or the vault is
            // locked); and refuse moving into a locked or hidden subtree
            // (MCP has no vault key to encrypt-on-place a *different*
            // plaintext note landing there, and a hidden destination would
            // just make this note vanish from MCP the instant it moves).
            let protected = writable_protected(store, &id)?;
            if let Some(dest) = folder_id.as_ref() {
                if store.folder_chain_has_lock(dest.as_deref())? {
                    return Err("folder is locked".into());
                }
                if store.folder_chain_has_mcp_hidden(dest.as_deref())? {
                    return Err("folder hidden from MCP".into());
                }
            }
            let mut note = store.get_note(&id)?.ok_or("note not found")?;
            if let Some(content) = s("content") {
                let html = to_html(&content, s("format").as_deref());
                if protected {
                    // Full replace — no need to decrypt the old content first.
                    store.write_protected(&id, &html)?;
                } else {
                    note.content = html;
                    note.updated_at = store.now_ms();
                    store.save(&note)?;
                }
            }
            // Move only when a group param was present (either key).
            if let Some(folder_id) = folder_id {
                store.set_folder(&id, folder_id.as_deref())?;
                note.folder_id = folder_id;
            }
            store.emit_changed();
            let folders = store.list_folders()?;
            Ok(json!({
                "id": note.id,
                "group": group_json(&folders, note.folder_id.as_deref()),
                "status": status_of(&note),
            })
            .to_string())
        }
        "create_group" => {
            if !allow_write {
                return Err("writing disabled".into());
            }
            let name = s("name").ok_or("name is required")?;
            let parent = s("parentId");
            let f = store.create_folder(&name, parent.as_deref())?;
            let folders = store.list_folders()?;
            Ok(json!({
                "id": f.id,
                "name": f.name,
                "parentId": f.parent_id,
                "path": folder_path(&folders, &f.id),
            })
            .to_string())
        }
        "archive_note" => {
            if !allow_write {
                return Err("writing disabled".into());
            }
            let id = s("id").ok_or("id is required")?;
            if store.is_effectively_mcp_hidden(&id)? {
                return Err("note hidden from MCP".into());
            }
            store.set_archived(&id, true)?;
            store.emit_changed();
            Ok(json!({ "id": id, "status": "archived" }).to_string())
        }
        "delete_note" => {
            if !allow_write {
                return Err("writing disabled".into());
            }
            let id = s("id").ok_or("id is required")?;
            if store.is_effectively_mcp_hidden(&id)? {
                return Err("note hidden from MCP".into());
            }
            store.trash(&id, store.now_ms())?;
            store.emit_changed();
            Ok(json!({ "id": id, "status": "trashed" }).to_string())
        }
        "restore_note" => {
            if !allow_write {
                return Err("writing disabled".into());
            }
            let id = s("id").ok_or("id is required")?;
            if store.is_effectively_mcp_hidden(&id)? {
                return Err("note hidden from MCP".into());
            }
            store.untrash(&id)?;
            store.set_archived(&id, false)?;
            store.emit_changed();
            Ok(json!({ "id": id, "status": "active" }).to_string())
        }
        _ => Err(format!("unknown tool {name}")),
    }
}

/// Returns None for notifications (no response).
pub fn handle_rpc(
    req: &Value,
    store: &dyn NoteStore,
    allow_write: bool,
    version: &str,
) -> Option<Value> {
    let id = req.get("id").cloned().unwrap_or(Value::Null);
    match req.get("method").and_then(|m| m.as_str()).unwrap_or("") {
        "initialize" => Some(ok(
            &id,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {}, "resources": {} },
                "serverInfo": { "name": "Notefix", "version": version }
            }),
        )),
        "notifications/initialized" => None,
        "ping" => Some(ok(&id, json!({}))),
        "tools/list" => Some(ok(&id, json!({ "tools": tool_defs() }))),
        "tools/call" => {
            let name = req
                .pointer("/params/name")
                .and_then(|n| n.as_str())
                .unwrap_or("");
            let args = req
                .pointer("/params/arguments")
                .cloned()
                .unwrap_or(json!({}));
            Some(match call_tool(name, &args, store, allow_write) {
                Ok(text) => ok(
                    &id,
                    json!({ "content": [{ "type": "text", "text": text }] }),
                ),
                Err(e) => ok(
                    &id,
                    json!({ "content": [{ "type": "text", "text": e }], "isError": true }),
                ),
            })
        }
        "resources/list" => {
            let mut res: Vec<Value> = Vec::new();
            for n in store.all_notes().unwrap_or_default() {
                if n.deleted_at.is_some() {
                    continue;
                }
                // Same visibility policy as the tools: omit hidden notes and
                // protected notes the current mcpProtectedAccess/vault state
                // doesn't permit. A permitted protected note comes back
                // decrypted. Fail closed — any error omits the note.
                let vn = match visible_for_read(store, &n) {
                    Ok(Some(vn)) => vn,
                    _ => continue,
                };
                res.push(json!({
                    "uri": format!("note://{}", vn.id),
                    "name": crate::mdconv::title_from_html(&vn.content),
                    "mimeType": "text/markdown"
                }));
            }
            Some(ok(&id, json!({ "resources": res })))
        }
        "resources/read" => {
            let uri = req
                .pointer("/params/uri")
                .and_then(|u| u.as_str())
                .unwrap_or("");
            // Match resources/list: filter out trashed notes, and apply the
            // same visibility policy — a hidden note, or a protected note the
            // current mcpProtectedAccess/vault state doesn't permit, reads as
            // "not found". A permitted protected note is served decrypted.
            // Fail closed — any error reads as not found.
            let visible = match store.get_note(uri.strip_prefix("note://").unwrap_or("")) {
                Ok(Some(note)) if note.deleted_at.is_none() => {
                    visible_for_read(store, &note).ok().flatten()
                }
                _ => None,
            };
            match visible {
                Some(vn) => Some(ok(
                    &id,
                    json!({ "contents": [{ "uri": uri, "mimeType": "text/markdown", "text": crate::mdconv::html_to_md(&vn.content) }] }),
                )),
                None => Some(rpc_err(&id, -32602, "note not found")),
            }
        }
        _ => Some(rpc_err(&id, -32601, "method not found")),
    }
}

pub struct StoreAccess {
    pub app: AppHandle,
}

impl NoteStore for StoreAccess {
    fn all_notes(&self) -> Result<Vec<Note>, String> {
        let st = self.app.state::<Mutex<crate::storage::Store>>();
        let store = st.lock().unwrap();
        store.load_all_notes().map_err(|e| e.to_string())
    }
    fn get_note(&self, id: &str) -> Result<Option<Note>, String> {
        Ok(self.all_notes()?.into_iter().find(|n| n.id == id))
    }
    fn save(&self, note: &Note) -> Result<(), String> {
        let st = self.app.state::<Mutex<crate::storage::Store>>();
        {
            let store = st.lock().unwrap();
            store.save_note(note).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    fn set_folder(&self, id: &str, folder_id: Option<&str>) -> Result<(), String> {
        let st = self.app.state::<Mutex<crate::storage::Store>>();
        let store = st.lock().unwrap();
        store.set_folder(id, folder_id).map_err(|e| e.to_string())
    }
    fn set_archived(&self, id: &str, archived: bool) -> Result<(), String> {
        let st = self.app.state::<Mutex<crate::storage::Store>>();
        let store = st.lock().unwrap();
        store.set_archived(id, archived).map_err(|e| e.to_string())
    }
    fn trash(&self, id: &str, ts: i64) -> Result<(), String> {
        let st = self.app.state::<Mutex<crate::storage::Store>>();
        let store = st.lock().unwrap();
        store.trash_note(id, ts).map_err(|e| e.to_string())
    }
    fn untrash(&self, id: &str) -> Result<(), String> {
        let st = self.app.state::<Mutex<crate::storage::Store>>();
        let store = st.lock().unwrap();
        store.restore_note(id).map_err(|e| e.to_string())
    }
    fn list_folders(&self) -> Result<Vec<Folder>, String> {
        let st = self.app.state::<Mutex<crate::storage::Store>>();
        let store = st.lock().unwrap();
        crate::folders::load_folders(&store.conn).map_err(|e| e.to_string())
    }
    fn create_folder(&self, name: &str, parent_id: Option<&str>) -> Result<Folder, String> {
        let id = self.new_id();
        let st = self.app.state::<Mutex<crate::storage::Store>>();
        {
            let store = st.lock().unwrap();
            crate::folders::create_folder(&store.conn, &id, name, parent_id)
                .map_err(|e| e.to_string())?;
        }
        // Read the created row back so position/defaults are accurate.
        self.list_folders()?
            .into_iter()
            .find(|f| f.id == id)
            .ok_or_else(|| "folder creation failed".to_string())
    }
    fn is_effectively_protected(&self, id: &str) -> Result<bool, String> {
        let st = self.app.state::<Mutex<crate::storage::Store>>();
        let store = st.lock().unwrap();
        store
            .is_effectively_protected(id)
            .map_err(|e| e.to_string())
    }
    fn folder_chain_has_lock(&self, folder_id: Option<&str>) -> Result<bool, String> {
        let st = self.app.state::<Mutex<crate::storage::Store>>();
        let store = st.lock().unwrap();
        store
            .folder_chain_has_lock(folder_id)
            .map_err(|e| e.to_string())
    }
    fn is_effectively_mcp_hidden(&self, id: &str) -> Result<bool, String> {
        let st = self.app.state::<Mutex<crate::storage::Store>>();
        let store = st.lock().unwrap();
        store
            .is_effectively_mcp_hidden(id)
            .map_err(|e| e.to_string())
    }
    fn folder_chain_has_mcp_hidden(&self, folder_id: Option<&str>) -> Result<bool, String> {
        let st = self.app.state::<Mutex<crate::storage::Store>>();
        let store = st.lock().unwrap();
        store
            .folder_chain_has_mcp_hidden(folder_id)
            .map_err(|e| e.to_string())
    }
    fn mcp_protected_access(&self) -> String {
        let st = self.app.state::<Mutex<crate::storage::Store>>();
        let store = st.lock().unwrap();
        crate::settings::get_string(&store.conn, "mcpProtectedAccess", "off")
    }
    fn vault_unlocked(&self) -> bool {
        let vst = self.app.state::<Mutex<crate::vault::state::VaultState>>();
        let vault = vst.lock().unwrap();
        vault.is_unlocked()
    }
    fn decrypt_protected(&self, id: &str, ciphertext: &str) -> Result<String, String> {
        let vst = self.app.state::<Mutex<crate::vault::state::VaultState>>();
        let vault = vst.lock().unwrap();
        let dek = vault.dek().ok_or_else(|| "vault locked".to_string())?;
        crate::commands::open_content(dek, id, ciphertext)
    }
    fn write_protected(&self, id: &str, plaintext_html: &str) -> Result<(), String> {
        let st = self.app.state::<Mutex<crate::storage::Store>>();
        let vst = self.app.state::<Mutex<crate::vault::state::VaultState>>();
        // Both locks held for the whole operation: writing the plaintext and
        // resealing it happen as one critical section, so no concurrent MCP
        // request can ever observe the note's content column as plaintext.
        let store = st.lock().unwrap();
        let vault = vst.lock().unwrap();
        let dek = vault.dek().ok_or_else(|| "vault locked".to_string())?;
        store
            .set_content_silent(id, plaintext_html)
            .map_err(|e| e.to_string())?;
        crate::commands::encrypt_note_in_place(&store, id, dek)
    }
    fn now_ms(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }
    fn new_id(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }
    fn emit_changed(&self) {
        let _ = self.app.emit("notes-changed", ());
    }
}

struct McpState {
    app: AppHandle,
    token: String,
    auth_required: bool,
    allow_write: bool,
    version: String,
}

static SHUTDOWN: OnceLock<Mutex<Option<tokio::sync::oneshot::Sender<()>>>> = OnceLock::new();

async fn mcp_route(
    axum::extract::State(state): axum::extract::State<Arc<McpState>>,
    headers: axum::http::HeaderMap,
    axum::Json(req): axum::Json<serde_json::Value>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    if state.auth_required {
        let want = format!("Bearer {}", state.token);
        let ok = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .map(|h| h == want)
            .unwrap_or(false);
        if !ok {
            return (axum::http::StatusCode::UNAUTHORIZED, "unauthorized").into_response();
        }
    }
    let access = StoreAccess {
        app: state.app.clone(),
    };
    match handle_rpc(&req, &access, state.allow_write, &state.version) {
        Some(resp) => axum::Json(resp).into_response(),
        None => axum::http::StatusCode::ACCEPTED.into_response(),
    }
}

pub async fn apply(
    app: AppHandle,
    enabled: bool,
    bind: String,
    port: u16,
    token: String,
    auth_required: bool,
    allow_write: bool,
) -> Result<(), String> {
    if let Some(slot) = SHUTDOWN.get() {
        if let Some(tx) = slot.lock().unwrap().take() {
            let _ = tx.send(());
        }
    }
    if !enabled {
        return Ok(());
    }
    let host = if bind == "external" {
        "0.0.0.0"
    } else {
        "127.0.0.1"
    };
    let addr: std::net::SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|e: std::net::AddrParseError| e.to_string())?;
    let version = app.package_info().version.to_string();
    let state = Arc::new(McpState {
        app,
        token,
        auth_required,
        allow_write,
        version,
    });
    let router = axum::Router::new()
        .route("/mcp", axum::routing::post(mcp_route))
        .with_state(state);
    // Bind with SO_REUSEADDR so a restart (e.g. reconfigure) doesn't fail with
    // EADDRINUSE while the previous listener's socket is still lingering.
    let socket = if addr.is_ipv4() {
        tokio::net::TcpSocket::new_v4()
    } else {
        tokio::net::TcpSocket::new_v6()
    }
    .map_err(|e| e.to_string())?;
    socket.set_reuseaddr(true).map_err(|e| e.to_string())?;
    socket.bind(addr).map_err(|e| e.to_string())?;
    let listener = socket.listen(1024).map_err(|e| e.to_string())?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    SHUTDOWN
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap()
        .replace(tx);
    tauri::async_runtime::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = rx.await;
            })
            .await;
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        notes: std::sync::Mutex<Vec<Note>>,
        folders: std::sync::Mutex<Vec<Folder>>,
        seq: std::sync::Mutex<u64>,
        // Feature 4 (MCP access to protected notes) test plumbing: the real
        // `StoreAccess` reads these from the managed `VaultState` / settings
        // table; the Fake just holds them directly. A "protected" note's
        // `content` in these tests is a fake ciphertext of the form
        // "CIPHER:<plaintext>" — `decrypt_protected`/`write_protected` below
        // strip/add that prefix instead of doing real AEAD, which is already
        // covered by the `vault`/`commands` unit tests.
        vault_unlocked: std::sync::Mutex<bool>,
        mcp_protected_access: std::sync::Mutex<String>,
    }
    impl NoteStore for Fake {
        fn all_notes(&self) -> Result<Vec<Note>, String> {
            Ok(self.notes.lock().unwrap().clone())
        }
        fn get_note(&self, id: &str) -> Result<Option<Note>, String> {
            Ok(self
                .notes
                .lock()
                .unwrap()
                .iter()
                .find(|n| n.id == id)
                .cloned())
        }
        fn save(&self, note: &Note) -> Result<(), String> {
            let mut v = self.notes.lock().unwrap();
            match v.iter_mut().find(|n| n.id == note.id) {
                // mirror save_note: only content/updated_at change on update
                Some(existing) => {
                    existing.content = note.content.clone();
                    existing.updated_at = note.updated_at;
                }
                None => v.push(note.clone()),
            }
            Ok(())
        }
        fn set_folder(&self, id: &str, folder_id: Option<&str>) -> Result<(), String> {
            let mut v = self.notes.lock().unwrap();
            let n = v.iter_mut().find(|n| n.id == id).ok_or("note not found")?;
            n.folder_id = folder_id.map(|s| s.to_string());
            Ok(())
        }
        fn set_archived(&self, id: &str, archived: bool) -> Result<(), String> {
            let mut v = self.notes.lock().unwrap();
            let n = v.iter_mut().find(|n| n.id == id).ok_or("note not found")?;
            n.archived = archived;
            Ok(())
        }
        fn trash(&self, id: &str, ts: i64) -> Result<(), String> {
            let mut v = self.notes.lock().unwrap();
            let n = v.iter_mut().find(|n| n.id == id).ok_or("note not found")?;
            n.deleted_at = Some(ts);
            Ok(())
        }
        fn untrash(&self, id: &str) -> Result<(), String> {
            let mut v = self.notes.lock().unwrap();
            let n = v.iter_mut().find(|n| n.id == id).ok_or("note not found")?;
            n.deleted_at = None;
            Ok(())
        }
        fn list_folders(&self) -> Result<Vec<Folder>, String> {
            Ok(self.folders.lock().unwrap().clone())
        }
        fn create_folder(&self, name: &str, parent_id: Option<&str>) -> Result<Folder, String> {
            let id = self.new_id();
            let f = Folder {
                id: id.clone(),
                name: name.to_string(),
                parent_id: parent_id.map(|s| s.to_string()),
                ..Default::default()
            };
            self.folders.lock().unwrap().push(f.clone());
            Ok(f)
        }
        fn is_effectively_protected(&self, id: &str) -> Result<bool, String> {
            let folder_id = {
                let notes = self.notes.lock().unwrap();
                let Some(n) = notes.iter().find(|n| n.id == id) else {
                    return Ok(false);
                };
                if n.protected {
                    return Ok(true);
                }
                n.folder_id.clone()
            };
            self.folder_chain_has_lock(folder_id.as_deref())
        }
        fn folder_chain_has_lock(&self, folder_id: Option<&str>) -> Result<bool, String> {
            let folders = self.folders.lock().unwrap();
            let mut fid = folder_id.map(|s| s.to_string());
            let mut visited = std::collections::HashSet::new();
            while let Some(id) = fid {
                if !visited.insert(id.clone()) {
                    break;
                }
                let Some(f) = folders.iter().find(|f| f.id == id) else {
                    break;
                };
                if f.locked {
                    return Ok(true);
                }
                fid = f.parent_id.clone();
            }
            Ok(false)
        }
        fn is_effectively_mcp_hidden(&self, id: &str) -> Result<bool, String> {
            let folder_id = {
                let notes = self.notes.lock().unwrap();
                let Some(n) = notes.iter().find(|n| n.id == id) else {
                    return Ok(false);
                };
                if n.mcp_hidden {
                    return Ok(true);
                }
                n.folder_id.clone()
            };
            self.folder_chain_has_mcp_hidden(folder_id.as_deref())
        }
        fn folder_chain_has_mcp_hidden(&self, folder_id: Option<&str>) -> Result<bool, String> {
            let folders = self.folders.lock().unwrap();
            let mut fid = folder_id.map(|s| s.to_string());
            let mut visited = std::collections::HashSet::new();
            while let Some(id) = fid {
                if !visited.insert(id.clone()) {
                    break;
                }
                let Some(f) = folders.iter().find(|f| f.id == id) else {
                    break;
                };
                if f.mcp_hidden {
                    return Ok(true);
                }
                fid = f.parent_id.clone();
            }
            Ok(false)
        }
        fn mcp_protected_access(&self) -> String {
            self.mcp_protected_access.lock().unwrap().clone()
        }
        fn vault_unlocked(&self) -> bool {
            *self.vault_unlocked.lock().unwrap()
        }
        fn decrypt_protected(&self, _id: &str, ciphertext: &str) -> Result<String, String> {
            if !self.vault_unlocked() {
                return Err("vault locked".into());
            }
            ciphertext
                .strip_prefix("CIPHER:")
                .map(str::to_string)
                .ok_or_else(|| "decrypt failed".into())
        }
        fn write_protected(&self, id: &str, plaintext_html: &str) -> Result<(), String> {
            if !self.vault_unlocked() {
                return Err("vault locked".into());
            }
            let mut v = self.notes.lock().unwrap();
            let n = v.iter_mut().find(|n| n.id == id).ok_or("note not found")?;
            n.content = format!("CIPHER:{plaintext_html}");
            n.protected = true;
            Ok(())
        }
        fn now_ms(&self) -> i64 {
            1_000
        }
        fn new_id(&self) -> String {
            let mut s = self.seq.lock().unwrap();
            *s += 1;
            format!("id{s}")
        }
        fn emit_changed(&self) {}
    }

    fn fake() -> Fake {
        Fake {
            notes: std::sync::Mutex::new(vec![Note {
                id: "a".into(),
                content: "<p>Hello world</p>".into(),
                updated_at: 1,
                ..Default::default()
            }]),
            folders: std::sync::Mutex::new(vec![]),
            seq: std::sync::Mutex::new(0),
            // Defaults match the app: vault locked, protected access "off" —
            // so existing (pre-Feature-4) tests keep exercising the unchanged
            // default-safe behavior without opting into anything.
            vault_unlocked: std::sync::Mutex::new(false),
            mcp_protected_access: std::sync::Mutex::new("off".to_string()),
        }
    }
    fn call(method: &str, params: Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params })
    }

    #[test]
    fn initialize_reports_version_and_caps() {
        let r = handle_rpc(&call("initialize", json!({})), &fake(), false, "1.2.3").unwrap();
        assert_eq!(r["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(r["result"]["serverInfo"]["version"], "1.2.3");
        assert!(r["result"]["capabilities"]["tools"].is_object());
    }
    #[test]
    fn tools_list_has_all_tools() {
        let r = handle_rpc(&call("tools/list", json!({})), &fake(), false, "v").unwrap();
        let names: Vec<String> = r["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        for expected in [
            "list_notes",
            "get_note",
            "search_notes",
            "list_groups",
            "create_note",
            "append_note",
            "update_note",
            "create_group",
            "archive_note",
            "delete_note",
            "restore_note",
        ] {
            assert!(
                names.contains(&expected.to_string()),
                "missing {expected}: {names:?}"
            );
        }
        // create_note advertises that content is Markdown:
        let cn = r["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == "create_note")
            .unwrap();
        assert!(cn["description"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("markdown"));
    }

    #[test]
    fn resources_read_returns_markdown() {
        let s = fake();
        let r = handle_rpc(
            &call("resources/read", json!({"uri":"note://a"})),
            &s,
            false,
            "v",
        )
        .unwrap();
        assert!(r["result"]["contents"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Hello world"));
        assert_eq!(r["result"]["contents"][0]["mimeType"], "text/markdown");
    }
    #[test]
    fn resources_read_trashed_note_is_not_found() {
        // resources/list already filters out trashed notes; resources/read
        // must be consistent and refuse to read one by URI too (the get_note
        // TOOL is unaffected and may still fetch any note by id).
        let s = fake();
        s.trash("a", 9).unwrap();
        let r = handle_rpc(
            &call("resources/read", json!({"uri":"note://a"})),
            &s,
            false,
            "v",
        )
        .unwrap();
        assert_eq!(r["error"]["message"], "note not found");
    }

    // The resources/* surface must apply the same visibility policy as the
    // tools — otherwise a client could enumerate/read hidden or protected
    // notes by URI, bypassing the tool-level checks.
    #[test]
    fn resources_omit_hidden_note() {
        let s = fake();
        s.notes.lock().unwrap()[0].mcp_hidden = true;

        let list = handle_rpc(&call("resources/list", json!({})), &s, false, "v").unwrap();
        assert!(
            list["result"]["resources"].as_array().unwrap().is_empty(),
            "got: {list}"
        );
        let read = handle_rpc(
            &call("resources/read", json!({"uri":"note://a"})),
            &s,
            false,
            "v",
        )
        .unwrap();
        assert_eq!(read["error"]["message"], "note not found");
    }

    #[test]
    fn resources_omit_protected_note_when_access_off() {
        let s = fake();
        {
            let mut notes = s.notes.lock().unwrap();
            notes[0].protected = true;
            notes[0].content = "CIPHER:<p>Hello world</p>".into();
        }
        // mcpProtectedAccess defaults to "off".

        let list = handle_rpc(&call("resources/list", json!({})), &s, false, "v").unwrap();
        assert!(
            list["result"]["resources"].as_array().unwrap().is_empty(),
            "got: {list}"
        );
        let read = handle_rpc(
            &call("resources/read", json!({"uri":"note://a"})),
            &s,
            false,
            "v",
        )
        .unwrap();
        assert_eq!(read["error"]["message"], "note not found");
    }

    #[test]
    fn resources_serve_protected_note_decrypted_when_access_read_unlocked() {
        let s = fake();
        {
            let mut notes = s.notes.lock().unwrap();
            notes[0].protected = true;
            notes[0].content = "CIPHER:<p>Hello world</p>".into();
        }
        *s.mcp_protected_access.lock().unwrap() = "read".to_string();
        *s.vault_unlocked.lock().unwrap() = true;

        let list = handle_rpc(&call("resources/list", json!({})), &s, false, "v").unwrap();
        assert_eq!(
            list["result"]["resources"].as_array().unwrap().len(),
            1,
            "got: {list}"
        );
        let read = handle_rpc(
            &call("resources/read", json!({"uri":"note://a"})),
            &s,
            false,
            "v",
        )
        .unwrap();
        assert!(
            read["result"]["contents"][0]["text"]
                .as_str()
                .unwrap()
                .contains("Hello world"),
            "got: {read}"
        );
    }

    #[test]
    fn notification_has_no_response() {
        assert!(handle_rpc(
            &call("notifications/initialized", json!({})),
            &fake(),
            false,
            "v"
        )
        .is_none());
    }
    #[test]
    fn html_text_roundtrip_helpers() {
        assert_eq!(html_to_text("<p>Hi</p><p>there</p>"), "Hi\nthere");
    }

    #[test]
    fn status_of_derives_from_fields() {
        let mut n = Note {
            id: "x".into(),
            ..Default::default()
        };
        assert_eq!(status_of(&n), "active");
        n.archived = true;
        assert_eq!(status_of(&n), "archived");
        n.deleted_at = Some(1);
        assert_eq!(status_of(&n), "trashed"); // trashed wins over archived
    }

    #[test]
    fn folder_path_walks_parents() {
        let folders = vec![
            Folder {
                id: "p".into(),
                name: "Work".into(),
                ..Default::default()
            },
            Folder {
                id: "c".into(),
                name: "Proj".into(),
                parent_id: Some("p".into()),
                ..Default::default()
            },
        ];
        assert_eq!(folder_path(&folders, "c"), "Work/Proj");
        assert_eq!(folder_path(&folders, "p"), "Work");
    }

    #[test]
    fn resolve_group_by_id_name_and_errors() {
        let s = fake();
        let f = s.create_folder("Home", None).unwrap();
        // by id
        assert_eq!(
            resolve_group(&s, Some(f.id.as_str()), None).unwrap(),
            Some(f.id.clone())
        );
        // by name (case-insensitive)
        assert_eq!(
            resolve_group(&s, None, Some("home")).unwrap(),
            Some(f.id.clone())
        );
        // neither -> None
        assert_eq!(resolve_group(&s, None, None).unwrap(), None);
        // unknown id
        assert!(resolve_group(&s, Some("nope"), None).is_err());
        // unknown name
        assert!(resolve_group(&s, None, Some("ghost")).is_err());
        // both -> error
        assert!(resolve_group(&s, Some(f.id.as_str()), Some("Home")).is_err());
    }

    #[test]
    fn resolve_group_ambiguous_name_errors() {
        let s = fake();
        s.create_folder("Dup", None).unwrap();
        s.create_folder("Dup", None).unwrap();
        let e = resolve_group(&s, None, Some("Dup")).unwrap_err();
        assert!(e.contains("ambiguous"), "got: {e}");
    }

    fn call_json(store: &dyn NoteStore, name: &str, args: Value, allow_write: bool) -> Value {
        let text = call_tool(name, &args, store, allow_write).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn list_notes_returns_summaries_with_group_and_status() {
        let s = fake();
        let f = s.create_folder("Work", None).unwrap();
        s.save(&Note {
            id: "b".into(),
            content: "<p>Second</p>".into(),
            folder_id: Some(f.id.clone()),
            updated_at: 2,
            ..Default::default()
        })
        .unwrap();
        let arr = call_json(&s, "list_notes", json!({}), false);
        let items = arr.as_array().unwrap();
        assert_eq!(items.len(), 2); // "a" (seed) + "b", both active
        let b = items.iter().find(|i| i["id"] == "b").unwrap();
        assert_eq!(b["group"]["name"], "Work");
        assert_eq!(b["contentType"], "markdown");
        assert_eq!(b["status"], "active");
        assert!(b["title"].as_str().unwrap().contains("Second"));
    }

    #[test]
    fn list_notes_status_filter() {
        let s = fake();
        s.save(&Note {
            id: "arch".into(),
            content: "<p>x</p>".into(),
            archived: true,
            updated_at: 2,
            ..Default::default()
        })
        .unwrap();
        s.save(&Note {
            id: "del".into(),
            content: "<p>y</p>".into(),
            deleted_at: Some(9),
            updated_at: 3,
            ..Default::default()
        })
        .unwrap();
        let active = call_json(&s, "list_notes", json!({}), false);
        assert_eq!(active.as_array().unwrap().len(), 1); // only seed "a"
        let archived = call_json(&s, "list_notes", json!({"status":"archived"}), false);
        assert_eq!(archived.as_array().unwrap()[0]["id"], "arch");
        let trashed = call_json(&s, "list_notes", json!({"status":"trashed"}), false);
        assert_eq!(trashed.as_array().unwrap()[0]["id"], "del");
        let all = call_json(&s, "list_notes", json!({"status":"all"}), false);
        assert_eq!(all.as_array().unwrap().len(), 3);
    }

    #[test]
    fn list_notes_group_filter() {
        let s = fake();
        let f = s.create_folder("G", None).unwrap();
        s.save(&Note {
            id: "in".into(),
            content: "<p>x</p>".into(),
            folder_id: Some(f.id.clone()),
            updated_at: 2,
            ..Default::default()
        })
        .unwrap();
        let arr = call_json(&s, "list_notes", json!({"groupId": f.id}), false);
        let items = arr.as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "in");
    }

    #[test]
    fn get_note_returns_markdown_content() {
        let s = fake();
        s.save(&Note {
            id: "m".into(),
            content: "<h1>Title</h1><p>body</p>".into(),
            updated_at: 2,
            ..Default::default()
        })
        .unwrap();
        let v = call_json(&s, "get_note", json!({"id":"m"}), false);
        assert_eq!(v["contentType"], "markdown");
        assert!(v["content"].as_str().unwrap().contains("# Title"));
        assert!(v.get("pinned").is_some());
    }

    #[test]
    fn get_note_html_format() {
        let s = fake();
        let v = call_json(&s, "get_note", json!({"id":"a","format":"html"}), false);
        assert_eq!(v["contentType"], "html");
        assert!(v["content"]
            .as_str()
            .unwrap()
            .contains("<p>Hello world</p>"));
    }

    #[test]
    fn get_note_missing_errors() {
        let s = fake();
        assert!(call_tool("get_note", &json!({"id":"zzz"}), &s, false).is_err());
    }

    #[test]
    fn search_notes_matches_and_snippets() {
        let s = fake();
        let arr = call_json(&s, "search_notes", json!({"query":"hello"}), false);
        let items = arr.as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "a");
        assert!(items[0]["snippet"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("hello"));
    }

    #[test]
    fn snippet_does_not_panic_on_expanding_lowercase() {
        // 'İ' (U+0130) lowercases to TWO chars ("i" + a combining dot
        // above), so `lower` can have more chars than `chars` (the
        // original). With a long-enough run of 'İ' immediately before a
        // late match, the char index computed from `lower`'s bytes minus 40
        // exceeds `chars.len()`, which used to panic the `chars[start..end]`
        // slice. 42 copies contribute 42 extra chars once lowered, enough to
        // push `start` (44) past `chars.len()` (43) for a match on the very
        // last character.
        let plain = format!("{}z", "İ".repeat(42));
        let q = plain.to_lowercase();
        // Must not panic; clamped to an empty/short in-bounds result instead.
        let _ = snippet(&plain, &q[q.len() - 1..]);
    }

    #[test]
    fn search_notes_snippet_does_not_panic_on_expanding_lowercase() {
        // Same expanding-lowercase scenario, exercised through the actual
        // search_notes path (query lowercased once, then passed to snippet).
        let s = fake();
        let content = format!("<p>{}z</p>", "İ".repeat(42));
        s.save(&Note {
            id: "tr".into(),
            content,
            updated_at: 2,
            ..Default::default()
        })
        .unwrap();
        let arr = call_json(&s, "search_notes", json!({"query":"z"}), false);
        let items = arr.as_array().unwrap();
        assert!(items.iter().any(|i| i["id"] == "tr"), "got: {items:?}");
    }

    #[test]
    fn list_groups_returns_tree_fields() {
        let s = fake();
        let p = s.create_folder("P", None).unwrap();
        s.create_folder("C", Some(p.id.as_str())).unwrap();
        let arr = call_json(&s, "list_groups", json!({}), false);
        let items = arr.as_array().unwrap();
        assert_eq!(items.len(), 2);
        let c = items.iter().find(|i| i["name"] == "C").unwrap();
        assert_eq!(c["parentId"], p.id);
        assert_eq!(c["path"], "P/C");
    }

    #[test]
    fn create_note_converts_markdown_and_sets_group() {
        let s = fake();
        let f = s.create_folder("Home", None).unwrap();
        let v = call_json(
            &s,
            "create_note",
            json!({"content":"# Hi\n- [ ] task","groupName":"Home"}),
            true,
        );
        assert_eq!(v["status"], "active");
        assert_eq!(v["group"]["name"], "Home");
        let id = v["id"].as_str().unwrap().to_string();
        let stored = s.get_note(&id).unwrap().unwrap();
        assert!(
            stored.content.contains("<h1>Hi</h1>"),
            "got: {}",
            stored.content
        );
        assert!(
            stored.content.contains(r#"data-type="taskItem""#),
            "got: {}",
            stored.content
        );
        assert_eq!(stored.folder_id.as_deref(), Some(f.id.as_str()));
    }

    #[test]
    fn create_note_blocked_when_writing_disabled() {
        let s = fake();
        assert_eq!(
            call_tool("create_note", &json!({"content":"x"}), &s, false).unwrap_err(),
            "writing disabled"
        );
    }

    #[test]
    fn create_note_unknown_group_errors() {
        let s = fake();
        assert!(call_tool(
            "create_note",
            &json!({"content":"x","groupName":"ghost"}),
            &s,
            true
        )
        .is_err());
    }

    #[test]
    fn create_note_format_html_is_not_stored_verbatim() {
        // Per product decision, writes must never store caller-supplied HTML
        // as-is: a "format":"html" override no longer exists on the write
        // path, so it falls through to the Markdown converter like any other
        // unrecognized format value, and raw HTML tags never survive intact.
        let s = fake();
        let v = call_json(
            &s,
            "create_note",
            json!({"content":"<script>alert(1)</script>","format":"html"}),
            true,
        );
        let id = v["id"].as_str().unwrap().to_string();
        let stored = s.get_note(&id).unwrap().unwrap();
        assert!(
            !stored.content.contains("<script>"),
            "raw HTML passthrough still present: {}",
            stored.content
        );
    }

    #[test]
    fn append_note_appends_converted_html() {
        let s = fake();
        let _ = call_tool(
            "append_note",
            &json!({"id":"a","text":"**more**"}),
            &s,
            true,
        )
        .unwrap();
        let stored = s.get_note("a").unwrap().unwrap();
        assert!(
            stored.content.contains("<p>Hello world</p>"),
            "got: {}",
            stored.content
        );
        assert!(
            stored.content.contains("<strong>more</strong>"),
            "got: {}",
            stored.content
        );
    }

    #[test]
    fn update_note_replaces_content_and_moves() {
        let s = fake();
        let f = s.create_folder("Dest", None).unwrap();
        let _ = call_tool(
            "update_note",
            &json!({"id":"a","content":"## New","groupName":"Dest"}),
            &s,
            true,
        )
        .unwrap();
        let stored = s.get_note("a").unwrap().unwrap();
        assert!(
            stored.content.contains("<h2>New</h2>"),
            "got: {}",
            stored.content
        );
        assert_eq!(stored.folder_id.as_deref(), Some(f.id.as_str()));
    }

    #[test]
    fn update_note_invalid_group_leaves_content_and_write_unapplied() {
        let s = fake();
        let before = s.get_note("a").unwrap().unwrap().content;
        let err = call_tool(
            "update_note",
            &json!({"id":"a","content":"## New","groupName":"ghost"}),
            &s,
            true,
        )
        .unwrap_err();
        assert!(err.contains("group not found"), "got: {err}");
        // The group is resolved before any save, so a bad group must leave
        // the note completely untouched -- content included.
        let after = s.get_note("a").unwrap().unwrap();
        assert_eq!(after.content, before);
        assert_eq!(after.folder_id, None);
    }

    #[test]
    fn update_note_move_only_keeps_content() {
        let s = fake();
        let f = s.create_folder("Dest", None).unwrap();
        let _ = call_tool("update_note", &json!({"id":"a","groupId": f.id}), &s, true).unwrap();
        let stored = s.get_note("a").unwrap().unwrap();
        assert_eq!(stored.content, "<p>Hello world</p>"); // unchanged
        assert_eq!(stored.folder_id.as_deref(), Some(f.id.as_str()));
    }

    #[test]
    fn create_group_returns_group_object() {
        let s = fake();
        let v = call_json(&s, "create_group", json!({"name":"Projects"}), true);
        assert_eq!(v["name"], "Projects");
        assert!(v["id"].as_str().unwrap().starts_with("id"));
        assert_eq!(v["path"], "Projects");
        assert_eq!(s.list_folders().unwrap().len(), 1);
    }

    #[test]
    fn archive_then_restore_roundtrip() {
        let s = fake();
        let a = call_json(&s, "archive_note", json!({"id":"a"}), true);
        assert_eq!(a["status"], "archived");
        assert!(s.get_note("a").unwrap().unwrap().archived);
        let r = call_json(&s, "restore_note", json!({"id":"a"}), true);
        assert_eq!(r["status"], "active");
        assert!(!s.get_note("a").unwrap().unwrap().archived);
    }

    #[test]
    fn delete_then_restore_roundtrip() {
        let s = fake();
        let d = call_json(&s, "delete_note", json!({"id":"a"}), true);
        assert_eq!(d["status"], "trashed");
        assert!(s.get_note("a").unwrap().unwrap().deleted_at.is_some());
        let r = call_json(&s, "restore_note", json!({"id":"a"}), true);
        assert_eq!(r["status"], "active");
        assert!(s.get_note("a").unwrap().unwrap().deleted_at.is_none());
    }

    #[test]
    fn lifecycle_tools_write_gated() {
        let s = fake();
        for t in [
            "create_group",
            "archive_note",
            "delete_note",
            "restore_note",
        ] {
            assert_eq!(
                call_tool(t, &json!({"id":"a","name":"n"}), &s, false).unwrap_err(),
                "writing disabled"
            );
        }
    }

    // ---- I2: MCP must be vault-aware (protected notes are off-limits) ----

    #[test]
    fn mcp_read_tools_exclude_protected_note() {
        // The seed note "a" is marked protected (ciphertext at rest in the real
        // store). It must be invisible across every read surface.
        let s = fake();
        s.notes.lock().unwrap()[0].protected = true;

        // get_note reports it as absent rather than leaking ciphertext/title.
        assert!(call_tool("get_note", &json!({"id":"a"}), &s, false).is_err());
        // list_notes (even status=all) omits it.
        let list = call_json(&s, "list_notes", json!({"status":"all"}), false);
        assert!(list.as_array().unwrap().is_empty(), "got: {list}");
        // search_notes never scans/matches its content.
        let search = call_json(&s, "search_notes", json!({"query":"hello"}), false);
        assert!(search.as_array().unwrap().is_empty(), "got: {search}");
    }

    #[test]
    fn mcp_write_tools_refuse_protected_note_and_leave_content() {
        let s = fake();
        s.notes.lock().unwrap()[0].protected = true;
        let before = s.get_note("a").unwrap().unwrap().content;

        let e = call_tool(
            "update_note",
            &json!({"id":"a","content":"## New"}),
            &s,
            true,
        )
        .unwrap_err();
        assert!(e.contains("protected"), "got: {e}");
        let e = call_tool("append_note", &json!({"id":"a","text":"x"}), &s, true).unwrap_err();
        assert!(e.contains("protected"), "got: {e}");

        // Neither write touched the stored (cipher)content.
        assert_eq!(s.get_note("a").unwrap().unwrap().content, before);
    }

    #[test]
    fn mcp_create_note_refuses_locked_folder() {
        let s = fake();
        s.folders.lock().unwrap().push(Folder {
            id: "locked".into(),
            name: "Locked".into(),
            locked: true,
            ..Default::default()
        });
        let e = call_tool(
            "create_note",
            &json!({"content":"secret","groupId":"locked"}),
            &s,
            true,
        )
        .unwrap_err();
        assert!(e.contains("locked"), "got: {e}");
        // Nothing was created — only the seed note remains.
        assert_eq!(s.all_notes().unwrap().len(), 1);
    }

    #[test]
    fn mcp_note_inside_locked_folder_is_hidden_and_write_refused() {
        // Physically plaintext (protected = false) but living in a locked
        // folder -> effectively protected via the folder-lock ancestor walk.
        let s = fake();
        s.folders.lock().unwrap().push(Folder {
            id: "locked".into(),
            name: "Locked".into(),
            locked: true,
            ..Default::default()
        });
        s.save(&Note {
            id: "inside".into(),
            content: "<p>hidden text</p>".into(),
            folder_id: Some("locked".into()),
            updated_at: 2,
            ..Default::default()
        })
        .unwrap();

        assert!(call_tool("get_note", &json!({"id":"inside"}), &s, false).is_err());
        let e = call_tool("append_note", &json!({"id":"inside","text":"x"}), &s, true).unwrap_err();
        assert!(e.contains("protected"), "got: {e}");
        // Also can't be moved INTO a locked folder from outside.
        let e = call_tool(
            "update_note",
            &json!({"id":"a","groupId":"locked"}),
            &s,
            true,
        )
        .unwrap_err();
        assert!(e.contains("locked"), "got: {e}");
    }

    // ---- Feature 3: "Hide from MCP" (schema v14) — LOCAL-only opt-out that
    // trumps everything, independent of protection. ----

    #[test]
    fn mcp_read_tools_exclude_hidden_note() {
        let s = fake();
        s.notes.lock().unwrap()[0].mcp_hidden = true;

        assert!(call_tool("get_note", &json!({"id":"a"}), &s, false).is_err());
        let list = call_json(&s, "list_notes", json!({"status":"all"}), false);
        assert!(list.as_array().unwrap().is_empty(), "got: {list}");
        let search = call_json(&s, "search_notes", json!({"query":"hello"}), false);
        assert!(search.as_array().unwrap().is_empty(), "got: {search}");
    }

    #[test]
    fn mcp_write_tools_refuse_hidden_note() {
        let s = fake();
        s.notes.lock().unwrap()[0].mcp_hidden = true;
        for (tool, args) in [
            ("append_note", json!({"id":"a","text":"x"})),
            ("update_note", json!({"id":"a","content":"## New"})),
            ("archive_note", json!({"id":"a"})),
            ("delete_note", json!({"id":"a"})),
            ("restore_note", json!({"id":"a"})),
        ] {
            let e = call_tool(tool, &args, &s, true).unwrap_err();
            assert!(e.contains("hidden"), "{tool} got: {e}");
        }
    }

    #[test]
    fn mcp_create_note_refuses_hidden_folder() {
        let s = fake();
        s.folders.lock().unwrap().push(Folder {
            id: "hidden".into(),
            name: "Hidden".into(),
            mcp_hidden: true,
            ..Default::default()
        });
        let e = call_tool(
            "create_note",
            &json!({"content":"secret","groupId":"hidden"}),
            &s,
            true,
        )
        .unwrap_err();
        assert!(e.contains("hidden"), "got: {e}");
        assert_eq!(s.all_notes().unwrap().len(), 1); // nothing created
    }

    #[test]
    fn mcp_note_inside_hidden_folder_is_excluded_and_write_refused() {
        // Physically not hidden itself, but living in an mcp_hidden folder ->
        // effectively hidden via the folder-chain walk (mirrors the analogous
        // locked-folder test above).
        let s = fake();
        s.folders.lock().unwrap().push(Folder {
            id: "hidden".into(),
            name: "Hidden".into(),
            mcp_hidden: true,
            ..Default::default()
        });
        s.save(&Note {
            id: "inside".into(),
            content: "<p>hidden text</p>".into(),
            folder_id: Some("hidden".into()),
            updated_at: 2,
            ..Default::default()
        })
        .unwrap();

        assert!(call_tool("get_note", &json!({"id":"inside"}), &s, false).is_err());
        let e = call_tool("append_note", &json!({"id":"inside","text":"x"}), &s, true).unwrap_err();
        assert!(e.contains("hidden"), "got: {e}");
        // Also can't be moved INTO a hidden folder from outside.
        let e = call_tool(
            "update_note",
            &json!({"id":"a","groupId":"hidden"}),
            &s,
            true,
        )
        .unwrap_err();
        assert!(e.contains("hidden"), "got: {e}");
    }

    #[test]
    fn mcp_hidden_trumps_protected_even_with_readwrite_access() {
        // Even wide-open protected-notes access must not resurrect a note
        // that's also marked hidden-from-MCP — hidden trumps everything.
        let s = fake();
        {
            let mut notes = s.notes.lock().unwrap();
            notes[0].mcp_hidden = true;
            notes[0].protected = true;
            notes[0].content = "CIPHER:<p>Hello world</p>".into();
        }
        *s.mcp_protected_access.lock().unwrap() = "readwrite".to_string();
        *s.vault_unlocked.lock().unwrap() = true;

        assert!(call_tool("get_note", &json!({"id":"a"}), &s, false).is_err());
        let e = call_tool(
            "update_note",
            &json!({"id":"a","content":"## New"}),
            &s,
            true,
        )
        .unwrap_err();
        assert!(e.contains("hidden"), "got: {e}");
    }

    // ---- Feature 4: MCP access to protected notes (`mcpProtectedAccess`) —
    // the full off/read/readwrite × locked/unlocked matrix. "off" (any vault
    // state) is exercised by `mcp_read_tools_exclude_protected_note` and
    // `mcp_write_tools_refuse_protected_note_and_leave_content` above:
    // `fake()`'s defaults are access="off"/vault locked, so those two tests
    // already prove the default-safe behavior is unchanged. ----

    #[test]
    fn mcp_protected_access_read_unlocked_returns_decrypted_content() {
        let s = fake();
        {
            let mut notes = s.notes.lock().unwrap();
            notes[0].protected = true;
            notes[0].content = "CIPHER:<p>Hello world</p>".into();
        }
        *s.mcp_protected_access.lock().unwrap() = "read".to_string();
        *s.vault_unlocked.lock().unwrap() = true;

        let v = call_json(&s, "get_note", json!({"id":"a"}), false);
        assert!(
            v["content"].as_str().unwrap().contains("Hello world"),
            "got: {v}"
        );

        let list = call_json(&s, "list_notes", json!({"status":"all"}), false);
        assert_eq!(list.as_array().unwrap().len(), 1, "got: {list}");

        let search = call_json(&s, "search_notes", json!({"query":"hello"}), false);
        assert_eq!(search.as_array().unwrap().len(), 1, "got: {search}");
    }

    #[test]
    fn mcp_protected_access_read_locked_excludes_protected_note() {
        let s = fake();
        {
            let mut notes = s.notes.lock().unwrap();
            notes[0].protected = true;
            notes[0].content = "CIPHER:<p>Hello world</p>".into();
        }
        *s.mcp_protected_access.lock().unwrap() = "read".to_string();
        // Vault stays locked — "read" alone is not enough.

        assert!(call_tool("get_note", &json!({"id":"a"}), &s, false).is_err());
        let list = call_json(&s, "list_notes", json!({"status":"all"}), false);
        assert!(list.as_array().unwrap().is_empty(), "got: {list}");
    }

    #[test]
    fn mcp_protected_access_readwrite_unlocked_write_succeeds_and_reseals() {
        let s = fake();
        {
            let mut notes = s.notes.lock().unwrap();
            notes[0].protected = true;
            notes[0].content = "CIPHER:<p>Hello world</p>".into();
        }
        *s.mcp_protected_access.lock().unwrap() = "readwrite".to_string();
        *s.vault_unlocked.lock().unwrap() = true;

        let v = call_json(
            &s,
            "update_note",
            json!({"id":"a","content":"## New"}),
            true,
        );
        assert_eq!(v["id"], "a");

        let stored = s.get_note("a").unwrap().unwrap();
        assert!(stored.protected, "must stay protected, got: {stored:?}");
        assert_ne!(
            stored.content, "<h2>New</h2>",
            "must never leave plaintext on disk"
        );
        assert!(
            stored.content.starts_with("CIPHER:") && stored.content.contains("<h2>New</h2>"),
            "must be resealed with the new content, got: {}",
            stored.content
        );
    }

    #[test]
    fn mcp_protected_access_readwrite_locked_refuses_write() {
        let s = fake();
        {
            let mut notes = s.notes.lock().unwrap();
            notes[0].protected = true;
            notes[0].content = "CIPHER:<p>Hello world</p>".into();
        }
        *s.mcp_protected_access.lock().unwrap() = "readwrite".to_string();
        // Vault stays locked — "readwrite" alone is not enough.

        let e = call_tool(
            "update_note",
            &json!({"id":"a","content":"## New"}),
            &s,
            true,
        )
        .unwrap_err();
        assert!(e.contains("protected"), "got: {e}");
        assert_eq!(
            s.get_note("a").unwrap().unwrap().content,
            "CIPHER:<p>Hello world</p>",
            "must be untouched"
        );
    }

    #[test]
    fn mcp_protected_access_decrypt_failure_is_treated_as_absent() {
        // Simulated corrupt/foreign ciphertext: missing the fake "CIPHER:"
        // marker, so `decrypt_protected` errors. Must never surface as
        // ciphertext or a partial result — treated exactly like "not found".
        let s = fake();
        {
            let mut notes = s.notes.lock().unwrap();
            notes[0].protected = true;
            notes[0].content = "not-actually-ciphertext".into();
        }
        *s.mcp_protected_access.lock().unwrap() = "read".to_string();
        *s.vault_unlocked.lock().unwrap() = true;

        assert!(call_tool("get_note", &json!({"id":"a"}), &s, false).is_err());
        let list = call_json(&s, "list_notes", json!({"status":"all"}), false);
        assert!(list.as_array().unwrap().is_empty(), "got: {list}");
        let search = call_json(&s, "search_notes", json!({"query":"hello"}), false);
        assert!(search.as_array().unwrap().is_empty(), "got: {search}");
    }
}
