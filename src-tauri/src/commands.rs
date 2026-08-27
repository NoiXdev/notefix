use std::sync::Mutex;

use base64::Engine;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::storage::{Note, NoteMeta, SearchHit, Store};
use crate::vault::aead::Dek;

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// What a `notefix://…` deep link should do.
#[derive(Debug, PartialEq)]
pub enum WidgetAction {
    NewNote,
    OpenNote(String),
    Auth,
}

/// Route a `notefix://…` deep link. Host `new` → new note; host `note` with a
/// non-empty path segment → open that id; everything else (browser auth
/// redirects, junk) → `Auth`, preserving the existing sign-in bridge.
pub fn parse_widget_url(url: &str) -> WidgetAction {
    let rest = match url.strip_prefix("notefix://") {
        Some(r) => r,
        None => return WidgetAction::Auth,
    };
    let path = rest.split(['?', '#']).next().unwrap_or("");
    let mut parts = path.split('/').filter(|s| !s.is_empty());
    match parts.next() {
        Some("new") => WidgetAction::NewNote,
        Some("note") => match parts.next() {
            Some(id) => WidgetAction::OpenNote(id.to_string()),
            None => WidgetAction::Auth,
        },
        _ => WidgetAction::Auth,
    }
}

/// Emit `notes-changed` to every window except the one that triggered the change,
/// mirroring the original Electron broadcast that excluded the sender.
fn broadcast_changed(app: &AppHandle, sender_label: &str) {
    let labels: Vec<String> = app.webview_windows().keys().cloned().collect();
    for label in labels {
        if label != sender_label {
            let _ = app.emit_to(label.as_str(), "notes-changed", ());
        }
    }
}

// Protected-notes vault: encrypt/decrypt around note content, and the
// protect/unprotect + folder-lock transition commands (Task 6).
//
// The physical-state invariant that makes this straightforward: `content` is
// ciphertext *iff* `notes.protected = 1`. `folders.locked` is a separate
// "intent" flag. Reads/writes below always consult `protected` directly
// (cheap); only the transition commands walk the folder tree.

/// Seals a note's plaintext HTML into a base64-encoded AEAD blob for storage
/// in the `notes.content` column. The note id is bound in as associated data
/// so a sealed blob can't be silently reattached to a different note's row.
fn seal_content(dek: &Dek, note_id: &str, html: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(crate::vault::aead::seal(
        dek,
        note_id.as_bytes(),
        html.as_bytes(),
    ))
}

/// Reverses `seal_content`: base64-decode, open the AEAD blob (checking the
/// note id as associated data), then validate the plaintext is UTF-8. Every
/// failure maps to a plain `String` — never key material or plaintext.
fn open_content(dek: &Dek, note_id: &str, stored: &str) -> Result<String, String> {
    let blob = base64::engine::general_purpose::STANDARD
        .decode(stored)
        .map_err(|e| e.to_string())?;
    let plaintext =
        crate::vault::aead::open(dek, note_id.as_bytes(), &blob).map_err(|e| e.to_string())?;
    String::from_utf8(plaintext).map_err(|e| e.to_string())
}

/// True if any ancestor folder of `note_id` (not counting the note's own
/// `protected` flag — see `Store::is_effectively_protected` for that) is
/// `locked`. Cycle-safe via a visited set, mirroring the walk in
/// `Store::is_effectively_protected`.
fn has_locked_ancestor_folder(store: &Store, note_id: &str) -> Result<bool, String> {
    use rusqlite::OptionalExtension;

    let folder_id: Option<String> = store
        .conn
        .query_row(
            "SELECT folder_id FROM notes WHERE id = ?1",
            [note_id],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .flatten();
    folder_chain_has_lock(store, folder_id.as_deref())
}

/// True if `starting_folder_id` or any of its ancestors is `locked`.
/// Cycle-safe via a visited set. Shared by `has_locked_ancestor_folder`
/// (starts from a note's *current* `folder_id`) and `notes_set_folder` /
/// `notes_reorder` (start from a move's *destination* `folder_id`, checked
/// before the move is performed). Thin `String`-error wrapper over
/// `Store::folder_chain_has_lock`.
fn folder_chain_has_lock(store: &Store, starting_folder_id: Option<&str>) -> Result<bool, String> {
    store
        .folder_chain_has_lock(starting_folder_id)
        .map_err(|e| e.to_string())
}

/// Encrypt one currently-plaintext note in place under `dek`: seal its content
/// (binding the note id as AEAD associated data), flip `protected`, mark it
/// dirty so the ciphertext + `protected = 1` propagate on sync, and purge its
/// now-defunct plaintext revision history. The single encrypt-on-transition
/// primitive shared by every "plaintext note enters a locked context" path
/// (`reconcile_folder_move`, `reconcile_reorder`, `note_set_protected(true)`,
/// `folder_set_locked(true)`), so all of them mark the row dirty identically.
fn encrypt_note_in_place(store: &Store, id: &str, dek: &Dek) -> Result<(), String> {
    let plaintext = store
        .load_note_content(id)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let sealed = seal_content(dek, id, &plaintext);
    store
        .set_content_silent(id, &sealed)
        .map_err(|e| e.to_string())?;
    store
        .set_note_protected(id, true)
        .map_err(|e| e.to_string())?;
    store
        .mark_note_dirty_if_syncing(id)
        .map_err(|e| e.to_string())?;
    crate::revisions::delete_revisions(&store.conn, id).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn notes_load(store: State<'_, Mutex<Store>>) -> Result<Vec<NoteMeta>, String> {
    let store = store.lock().map_err(|e| e.to_string())?;
    store.load_notes_meta().map_err(|e| e.to_string())
}

/// The full HTML content of one note (empty string if it no longer exists).
/// Protected notes require the vault to be unlocked — `Err("vault locked")`
/// otherwise — and are decrypted before returning.
#[tauri::command]
pub fn notes_load_one(
    store: State<'_, Mutex<Store>>,
    vault: VaultStateHandle<'_>,
    id: String,
) -> Result<String, String> {
    let store = store.lock().map_err(|e| e.to_string())?;
    let stored = match store.load_note_content(&id).map_err(|e| e.to_string())? {
        Some(c) => c,
        None => return Ok(String::new()),
    };
    if store.note_protected(&id).map_err(|e| e.to_string())? {
        let vault = vault.lock().map_err(|e| e.to_string())?;
        let dek = vault.dek().ok_or_else(|| "vault locked".to_string())?;
        open_content(dek, &id, &stored)
    } else {
        Ok(stored)
    }
}

/// Full-text search within the active context (title-first), with snippets.
/// Protected notes are excluded while the vault is locked — their `content`
/// is ciphertext, so a plaintext scan can't match it correctly anyway.
#[tauri::command]
pub fn notes_search(
    store: State<'_, Mutex<Store>>,
    vault: VaultStateHandle<'_>,
    query: String,
) -> Result<Vec<SearchHit>, String> {
    let exclude_protected = !vault.lock().map_err(|e| e.to_string())?.is_unlocked();
    let store = store.lock().map_err(|e| e.to_string())?;
    store
        .search_notes(&query, 50, exclude_protected)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn notes_save(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    vault: VaultStateHandle<'_>,
    note: Note,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        let protected = store
            .is_effectively_protected(&note.id)
            .map_err(|e| e.to_string())?;
        if protected {
            let vault = vault.lock().map_err(|e| e.to_string())?;
            let dek = vault.dek().ok_or_else(|| "vault locked".to_string())?;
            let mut sealed = note.clone();
            sealed.content = seal_content(dek, &note.id, &note.content);
            store.save_note(&sealed).map_err(|e| e.to_string())?;
            store
                .set_note_protected(&note.id, true)
                .map_err(|e| e.to_string())?;
            // Never persist a protected note's plaintext into the (unencrypted)
            // note_revisions table — skip revision history for this save, and
            // purge any plaintext revisions recorded before this transition
            // (no-op if already empty; safe to call on every protected save).
            crate::revisions::delete_revisions(&store.conn, &note.id).map_err(|e| e.to_string())?;
        } else {
            store.save_note(&note).map_err(|e| e.to_string())?;
            let limit = crate::settings::get_int(&store.conn, "revisionLimit", 50);
            crate::revisions::add_revision(&store.conn, &note.id, &note.content, limit)
                .map_err(|e| e.to_string())?;
        }
    }
    broadcast_changed(&app, webview.label());
    crate::tray::rebuild_menu(&app);
    Ok(())
}

#[tauri::command]
pub fn notes_delete(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    id: String,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        if store.sync_enabled {
            store.sync_delete_note(&id).map_err(|e| e.to_string())?;
        } else if crate::settings::get_bool_default(&store.conn, "trashEnabled", true) {
            store.trash_note(&id, now_ms()).map_err(|e| e.to_string())?;
        } else {
            store.delete_note(&id).map_err(|e| e.to_string())?;
        }
        crate::images::run_gc(&app, &store);
    }
    broadcast_changed(&app, webview.label());
    crate::tray::rebuild_menu(&app);
    Ok(())
}

#[tauri::command]
pub fn notes_restore(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    id: String,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        store.restore_note(&id).map_err(|e| e.to_string())?;
    }
    broadcast_changed(&app, webview.label());
    crate::tray::rebuild_menu(&app);
    Ok(())
}

#[tauri::command]
pub fn notes_purge(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    id: String,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        store.delete_note(&id).map_err(|e| e.to_string())?;
        crate::images::run_gc(&app, &store);
    }
    broadcast_changed(&app, webview.label());
    crate::tray::rebuild_menu(&app);
    Ok(())
}

#[tauri::command]
pub fn trash_load(store: State<'_, Mutex<Store>>) -> Result<Vec<NoteMeta>, String> {
    let store = store.lock().map_err(|e| e.to_string())?;
    store.load_trashed_meta().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn trash_empty(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        store.purge_trashed(None).map_err(|e| e.to_string())?;
        crate::images::run_gc(&app, &store);
    }
    broadcast_changed(&app, webview.label());
    crate::tray::rebuild_menu(&app);
    Ok(())
}

/// Open a frameless floating window for a note. Tauri requires unique labels,
/// so re-opening the same note focuses the existing window instead of duplicating.
#[tauri::command]
pub async fn open_note_window(app: AppHandle, note_id: String) -> Result<(), String> {
    let label = format!("note-{note_id}");
    // Separate note windows are a desktop concept; mobile is single-window.
    #[cfg(desktop)]
    {
        if let Some(win) = app.get_webview_window(&label) {
            win.set_focus().map_err(|e| e.to_string())?;
            return Ok(());
        }
        let url = format!("index.html?windowNoteId={note_id}");
        WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(url.into()))
            .title("Notefix")
            .inner_size(700.0, 820.0)
            .decorations(false)
            .build()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(desktop))]
    let _ = (&app, &label);
    Ok(())
}

#[tauri::command]
pub fn notes_set_pinned(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    id: String,
    pinned: bool,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        store.set_pinned(&id, pinned).map_err(|e| e.to_string())?;
    }
    broadcast_changed(&app, webview.label());
    crate::tray::rebuild_menu(&app);
    Ok(())
}

#[tauri::command]
pub fn settings_load(store: State<'_, Mutex<Store>>) -> Result<Vec<(String, String)>, String> {
    let store = store.lock().map_err(|e| e.to_string())?;
    crate::settings::load_settings(&store.conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn settings_set(
    store: State<'_, Mutex<Store>>,
    key: String,
    value: String,
) -> Result<(), String> {
    let store = store.lock().map_err(|e| e.to_string())?;
    crate::settings::set_setting(&store.conn, &key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn notes_set_archived(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    id: String,
    archived: bool,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        store
            .set_archived(&id, archived)
            .map_err(|e| e.to_string())?;
    }
    broadcast_changed(&app, webview.label());
    crate::tray::rebuild_menu(&app);
    Ok(())
}

#[tauri::command]
pub fn notes_set_color(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    id: String,
    color: String,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        store.set_color(&id, &color).map_err(|e| e.to_string())?;
    }
    broadcast_changed(&app, webview.label());
    crate::tray::rebuild_menu(&app);
    Ok(())
}

#[tauri::command]
pub fn export_notes(
    store: State<'_, Mutex<Store>>,
    path: String,
    ids: Vec<String>,
) -> Result<(), String> {
    let notes = {
        let store = store.lock().map_err(|e| e.to_string())?;
        store.load_notes().map_err(|e| e.to_string())?
    };
    let json = crate::export::notes_to_json(&notes, &ids).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn notes_set_due(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    id: String,
    due_at: Option<i64>,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        store.set_due(&id, due_at).map_err(|e| e.to_string())?;
    }
    broadcast_changed(&app, webview.label());
    crate::tray::rebuild_menu(&app);
    Ok(())
}

#[tauri::command]
pub fn note_stats(store: State<'_, Mutex<Store>>) -> Result<crate::stats::Stats, String> {
    let notes = {
        let store = store.lock().map_err(|e| e.to_string())?;
        store.load_notes().map_err(|e| e.to_string())?
    };
    Ok(crate::stats::compute(&notes))
}

#[tauri::command]
pub fn folders_load(store: State<'_, Mutex<Store>>) -> Result<Vec<crate::folders::Folder>, String> {
    let store = store.lock().map_err(|e| e.to_string())?;
    crate::folders::load_folders(&store.conn).map_err(|e| e.to_string())
}

fn notify(app: &AppHandle, webview: &WebviewWindow) {
    broadcast_changed(app, webview.label());
    crate::tray::rebuild_menu(app);
}

#[tauri::command]
pub fn folder_create(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    id: String,
    name: String,
    parent_id: Option<String>,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        crate::folders::create_folder(&store.conn, &id, &name, parent_id.as_deref())
            .map_err(|e| e.to_string())?;
        if store.sync_enabled {
            crate::folders::touch_folder(&store.conn, &id).map_err(|e| e.to_string())?;
        }
    }
    notify(&app, &webview);
    Ok(())
}

#[tauri::command]
pub fn folder_rename(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    id: String,
    name: String,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        crate::folders::rename_folder(&store.conn, &id, &name).map_err(|e| e.to_string())?;
        if store.sync_enabled {
            crate::folders::touch_folder(&store.conn, &id).map_err(|e| e.to_string())?;
        }
    }
    notify(&app, &webview);
    Ok(())
}

#[tauri::command]
pub fn folder_move(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    id: String,
    parent_id: Option<String>,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        crate::folders::move_folder(&store.conn, &id, parent_id.as_deref())
            .map_err(|e| e.to_string())?;
        if store.sync_enabled {
            crate::folders::touch_folder(&store.conn, &id).map_err(|e| e.to_string())?;
        }
    }
    notify(&app, &webview);
    Ok(())
}

#[tauri::command]
pub fn folder_delete(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    id: String,
    mode: String,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        if store.sync_enabled {
            crate::folders::sync_delete_folder(
                &store.conn,
                &id,
                crate::folders::DeleteMode::from_str(&mode),
            )
            .map_err(|e| e.to_string())?;
        } else {
            crate::folders::delete_folder(
                &store.conn,
                &id,
                crate::folders::DeleteMode::from_str(&mode),
            )
            .map_err(|e| e.to_string())?;
        }
        crate::images::run_gc(&app, &store);
    }
    notify(&app, &webview);
    Ok(())
}

/// Core reconciliation logic behind `notes_set_folder`, factored out so it's
/// unit-testable without a Tauri `State` harness: `dek` is `None` to
/// represent a locked vault, `Some(&dek)` unlocked.
///
/// Moves only ever ADD protection, never remove it:
/// - Already-encrypted notes stay encrypted regardless of destination — the
///   safe direction; moving an encrypted note out of a locked folder does
///   NOT auto-decrypt it (the user can explicitly unprotect it).
/// - A currently-plaintext note moving into a location with a locked
///   ancestor folder must become encrypted. That check — and the
///   `dek.is_some()` requirement it implies — happens BEFORE the move is
///   performed, so a locked vault never leaves the note relocated into a
///   locked folder while still plaintext: this returns `Err("vault
///   locked")` and the note stays where (and as) it was.
/// - A plaintext note moving into a non-locked location is an unchanged
///   plain move.
/// - The freshly-encrypted note's revision history is purged (it was
///   plaintext, so no plaintext survives a transition to encrypted).
fn reconcile_folder_move(
    store: &Store,
    id: &str,
    folder_id: Option<&str>,
    dek: Option<&Dek>,
) -> Result<(), String> {
    let already_protected = store.note_protected(id).map_err(|e| e.to_string())?;
    let needs_encryption = !already_protected && folder_chain_has_lock(store, folder_id)?;

    if needs_encryption {
        let dek = dek.ok_or_else(|| "vault locked".to_string())?;
        store.set_folder(id, folder_id).map_err(|e| e.to_string())?;
        encrypt_note_in_place(store, id, dek)?;
    } else {
        store.set_folder(id, folder_id).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Moves a note to a (possibly different) folder — see
/// `reconcile_folder_move` for the encrypt-on-move-in policy.
#[tauri::command]
pub fn notes_set_folder(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    vault: VaultStateHandle<'_>,
    id: String,
    folder_id: Option<String>,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        let vault = vault.lock().map_err(|e| e.to_string())?;
        reconcile_folder_move(&store, &id, folder_id.as_deref(), vault.dek())?;
    }
    notify(&app, &webview);
    Ok(())
}

/// Core reconciliation behind `notes_reorder`, factored out (like
/// `reconcile_folder_move`) so it's unit-testable without a Tauri `State`
/// harness: `dek` is `None` for a locked vault, `Some(&dek)` unlocked.
///
/// Drag-and-drop reorder assigns every id to `folder_id`. If that destination
/// has a locked ancestor, any currently-plaintext note among `ids` would land
/// inside a locked subtree as plaintext-at-rest — the same leak
/// `reconcile_folder_move` prevents for the context-menu move path. So:
/// - Already-protected (ciphertext) notes just get repositioned — the safe
///   direction, never auto-decrypted.
/// - A plaintext note entering the locked destination must be encrypted with
///   the SAME primitive the move path uses (`encrypt_note_in_place`).
/// - If the vault is locked and any plaintext note would enter the locked
///   subtree, the WHOLE operation is refused (`Err("vault locked")`) before any
///   row is touched — never a half-applied reorder that strands plaintext in a
///   locked folder.
/// - A non-locked destination is an unchanged plain reorder.
fn reconcile_reorder(
    store: &Store,
    folder_id: Option<&str>,
    ids: &[String],
    dek: Option<&Dek>,
) -> Result<(), String> {
    if !folder_chain_has_lock(store, folder_id)? {
        return store
            .reorder_notes(folder_id, ids)
            .map_err(|e| e.to_string());
    }

    // Destination is inside a locked subtree. Which ids are currently plaintext
    // (physically unencrypted) and therefore need sealing on entry? A missing
    // id (stale drag payload) is skipped — `reorder_notes` no-ops it too.
    use rusqlite::OptionalExtension;
    let mut to_encrypt: Vec<&String> = Vec::new();
    for id in ids {
        let protected: Option<bool> = store
            .conn
            .query_row("SELECT protected FROM notes WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()
            .map_err(|e| e.to_string())?;
        if protected == Some(false) {
            to_encrypt.push(id);
        }
    }

    // Refuse the entire op up front if we'd strand a plaintext note in a locked
    // folder without an unlocked DEK — nothing is mutated on this path.
    if !to_encrypt.is_empty() && dek.is_none() {
        return Err("vault locked".to_string());
    }

    store
        .reorder_notes(folder_id, ids)
        .map_err(|e| e.to_string())?;
    if let Some(dek) = dek {
        for id in to_encrypt {
            encrypt_note_in_place(store, id, dek)?;
        }
    }
    Ok(())
}

/// Reorders/repositions notes within a folder — see `reconcile_reorder` for the
/// encrypt-on-drop-into-locked policy that keeps drag-and-drop as safe as the
/// context-menu move path (`notes_set_folder`).
#[tauri::command]
pub fn notes_reorder(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    vault: VaultStateHandle<'_>,
    folder_id: Option<String>,
    ids: Vec<String>,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        let vault = vault.lock().map_err(|e| e.to_string())?;
        reconcile_reorder(&store, folder_id.as_deref(), &ids, vault.dek())?;
    }
    notify(&app, &webview);
    Ok(())
}

#[tauri::command]
pub fn folders_reorder(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    parent_id: Option<String>,
    ids: Vec<String>,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        crate::folders::reorder_folders(&store.conn, parent_id.as_deref(), &ids)
            .map_err(|e| e.to_string())?;
    }
    notify(&app, &webview);
    Ok(())
}

#[tauri::command]
pub fn folder_set_icon(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    id: String,
    icon: String,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        crate::folders::set_folder_icon(&store.conn, &id, &icon).map_err(|e| e.to_string())?;
        if store.sync_enabled {
            crate::folders::touch_folder(&store.conn, &id).map_err(|e| e.to_string())?;
        }
    }
    notify(&app, &webview);
    Ok(())
}

#[tauri::command]
pub fn folder_set_color(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    id: String,
    color: String,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        crate::folders::set_folder_color(&store.conn, &id, &color).map_err(|e| e.to_string())?;
        if store.sync_enabled {
            crate::folders::touch_folder(&store.conn, &id).map_err(|e| e.to_string())?;
        }
    }
    notify(&app, &webview);
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbLocationResult {
    pub mode: String,
    pub path: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextInfo {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub path: String,
    pub server_url: String,
    pub workspace_id: String,
    pub active: bool,
}

fn to_infos(reg: &crate::profiles::Registry) -> Vec<ContextInfo> {
    reg.contexts
        .iter()
        .map(|c| ContextInfo {
            id: c.id.clone(),
            label: c.label.clone(),
            kind: c.kind.clone(),
            path: c.path.clone(),
            server_url: c.server_url.clone(),
            workspace_id: c.workspace_id.clone(),
            active: c.id == reg.active_id,
        })
        .collect()
}

#[tauri::command]
pub fn contexts_list(
    reg: State<'_, Mutex<crate::profiles::Registry>>,
) -> Result<Vec<ContextInfo>, String> {
    let r = reg.lock().map_err(|e| e.to_string())?;
    Ok(to_infos(&r))
}

#[tauri::command]
pub fn context_add(
    app: AppHandle,
    reg: State<'_, Mutex<crate::profiles::Registry>>,
    store: State<'_, Mutex<Store>>,
    label: String,
) -> Result<Vec<ContextInfo>, String> {
    let id = uuid::Uuid::new_v4().to_string();
    // Each context lives in its own directory so its images (resolved as
    // <db-dir>/images) stay isolated from every other context.
    let dir = crate::config::contexts_dir(&app).join(&id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("notefix.db");
    // Initialise the new DB.
    {
        let s = Store::open(&path).map_err(|e| e.to_string())?;
        crate::migrate::run_migrations(&s.conn).map_err(|e| e.to_string())?;
    }
    let infos = {
        let mut r = reg.lock().map_err(|e| e.to_string())?;
        r.add(id.clone(), label, path.to_string_lossy().into_owned());
        r.set_active(&id)?;
        crate::profiles::save(&crate::config::profiles_path(&app), &r)
            .map_err(|e| e.to_string())?;
        to_infos(&r)
    };
    swap_store_to(&store, &path, false)?;
    broadcast_context_changed(&app);
    Ok(infos)
}

#[tauri::command]
pub fn context_switch(
    app: AppHandle,
    reg: State<'_, Mutex<crate::profiles::Registry>>,
    store: State<'_, Mutex<Store>>,
    id: String,
) -> Result<(), String> {
    let (path, kind) = {
        let mut r = reg.lock().map_err(|e| e.to_string())?;
        r.set_active(&id)?;
        let p = r.active().unwrap().path.clone();
        let kind = r.active().map(|c| c.kind.clone()).unwrap_or_default();
        crate::profiles::save(&crate::config::profiles_path(&app), &r)
            .map_err(|e| e.to_string())?;
        (p, kind)
    };
    swap_store_to(&store, std::path::Path::new(&path), kind == "server")?;
    broadcast_context_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn context_rename(
    app: AppHandle,
    reg: State<'_, Mutex<crate::profiles::Registry>>,
    id: String,
    label: String,
) -> Result<Vec<ContextInfo>, String> {
    let mut r = reg.lock().map_err(|e| e.to_string())?;
    r.rename(&id, label)?;
    crate::profiles::save(&crate::config::profiles_path(&app), &r).map_err(|e| e.to_string())?;
    Ok(to_infos(&r))
}

#[tauri::command]
pub fn context_remove(
    app: AppHandle,
    reg: State<'_, Mutex<crate::profiles::Registry>>,
    id: String,
    delete_file: bool,
) -> Result<Vec<ContextInfo>, String> {
    let (removed, infos) = {
        let mut r = reg.lock().map_err(|e| e.to_string())?;
        let removed = r.remove(&id)?;
        crate::profiles::save(&crate::config::profiles_path(&app), &r)
            .map_err(|e| e.to_string())?;
        (removed, to_infos(&r))
    };
    if delete_file {
        for ext in ["", "-wal", "-shm"] {
            let p = with_ext(std::path::Path::new(&removed.path), ext);
            let _ = std::fs::remove_file(p);
        }
    }
    // Server contexts keep their tokens in the keychain; drop them on removal.
    if removed.kind == "server" {
        let _ = crate::auth::clear_tokens(&removed.id);
    }
    Ok(infos)
}

/// Pending browser auth flows, keyed by the PKCE `state`. Lives only in memory:
/// a flow that is never completed is simply forgotten when the app exits.
pub struct PendingAuth {
    pub verifier: String,
    pub server_url: String,
    pub config: crate::auth::OAuthConfig,
}
pub type PendingAuthMap = Mutex<std::collections::HashMap<String, PendingAuth>>;

fn server_label(server_url: &str) -> String {
    url::Url::parse(server_url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| server_url.to_string())
}

/// Step 1 of add-server: discover the server's OAuth config, mint PKCE material,
/// stash it under its `state`, and return the browser authorize URL to open.
#[tauri::command]
pub async fn server_auth_begin(
    pending: State<'_, PendingAuthMap>,
    server_url: String,
) -> Result<String, String> {
    let server_url = crate::auth::normalize_server_url(&server_url);
    let config = crate::auth::fetch_oauth_config(&server_url).await?;
    let p = crate::auth::pkce();

    let mut authorize = url::Url::parse(&config.authorize_url).map_err(|e| e.to_string())?;
    {
        let mut q = authorize.query_pairs_mut();
        q.append_pair("response_type", "code");
        q.append_pair("client_id", &config.client_id);
        q.append_pair("redirect_uri", crate::auth::REDIRECT_URI);
        q.append_pair("code_challenge", &p.challenge);
        q.append_pair("code_challenge_method", "S256");
        q.append_pair("state", &p.state);
        if !config.scopes.is_empty() {
            q.append_pair("scope", &config.scopes.join(" "));
        }
    }
    let authorize = authorize.to_string();

    pending.lock().map_err(|e| e.to_string())?.insert(
        p.state,
        PendingAuth {
            verifier: p.verifier,
            server_url,
            config,
        },
    );
    Ok(authorize)
}

/// Step 2 of add-server: handle the `notefix://auth?code=…&state=…` callback —
/// validate state, exchange the code, store tokens in the keychain, and add a
/// server context (with its own local cache DB) as the active context.
#[tauri::command]
pub async fn server_auth_complete(
    app: AppHandle,
    reg: State<'_, Mutex<crate::profiles::Registry>>,
    store: State<'_, Mutex<Store>>,
    pending: State<'_, PendingAuthMap>,
    url: String,
) -> Result<Vec<ContextInfo>, String> {
    let parsed = url::Url::parse(&url).map_err(|e| e.to_string())?;
    let (mut code, mut state) = (None, None);
    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "code" => code = Some(v.into_owned()),
            "state" => state = Some(v.into_owned()),
            _ => {}
        }
    }
    let code = code.ok_or("missing code in callback")?;
    let state = state.ok_or("missing state in callback")?;

    // Validate + consume the pending flow (CSRF: unknown state is rejected).
    let pa = pending
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&state)
        .ok_or("unknown or expired auth state")?;

    let tokens = crate::auth::exchange_code(
        &pa.config.token_url,
        &pa.config.client_id,
        &code,
        &pa.verifier,
    )
    .await?;

    // A server context still owns a local cache DB (its sync engine lands in C1).
    let id = uuid::Uuid::new_v4().to_string();
    let dir = crate::config::contexts_dir(&app).join(&id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("notefix.db");
    {
        let s = Store::open(&path).map_err(|e| e.to_string())?;
        crate::migrate::run_migrations(&s.conn).map_err(|e| e.to_string())?;
    }
    crate::auth::store_tokens(&id, &tokens)?;

    let label = server_label(&pa.server_url);
    let infos = {
        let mut r = reg.lock().map_err(|e| e.to_string())?;
        r.add_server(
            id.clone(),
            label,
            path.to_string_lossy().into_owned(),
            pa.server_url,
        );
        r.set_active(&id)?;
        crate::profiles::save(&crate::config::profiles_path(&app), &r)
            .map_err(|e| e.to_string())?;
        to_infos(&r)
    };
    swap_store_to(&store, &path, true)?;
    broadcast_context_changed(&app);
    Ok(infos)
}

// Lock convention: never hold the Store and Registry locks simultaneously; if ever needed, lock Store before Registry.
fn swap_store_to(
    store: &State<'_, Mutex<Store>>,
    path: &std::path::Path,
    sync_enabled: bool,
) -> Result<(), String> {
    let mut s = store.lock().map_err(|e| e.to_string())?;
    let opened = Store::open(path).map_err(|e| e.to_string())?;
    s.conn = opened.conn;
    s.sync_enabled = sync_enabled;
    crate::migrate::run_migrations(&s.conn).map_err(|e| e.to_string())?;
    Ok(())
}

fn broadcast_context_changed(app: &AppHandle) {
    let labels: Vec<String> = app.webview_windows().keys().cloned().collect();
    for label in labels {
        let _ = app.emit_to(label.as_str(), "context-changed", ());
    }
    crate::tray::rebuild_menu(app);
}

fn with_ext(path: &std::path::Path, ext: &str) -> std::path::PathBuf {
    if ext.is_empty() {
        path.to_path_buf()
    } else {
        std::path::PathBuf::from(format!("{}{}", path.to_string_lossy(), ext))
    }
}

fn move_file(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            std::fs::copy(from, to)?;
            std::fs::remove_file(from)
        }
    }
}

#[tauri::command]
pub fn get_db_path(app: AppHandle) -> String {
    crate::config::read_db_path(&app)
        .to_string_lossy()
        .into_owned()
}

#[tauri::command]
pub fn set_db_location(
    app: AppHandle,
    store: State<'_, Mutex<Store>>,
    folder: String,
) -> Result<DbLocationResult, String> {
    let target = std::path::PathBuf::from(&folder).join("notefix.db");
    let current = crate::config::read_db_path(&app);

    let mode = if target.exists() {
        "switched"
    } else {
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        // Release the DB file so it can be moved.
        {
            let mut s = store.lock().map_err(|e| e.to_string())?;
            s.conn = rusqlite::Connection::open_in_memory().map_err(|e| e.to_string())?;
        }
        for ext in ["", "-wal", "-shm"] {
            let from = with_ext(&current, ext);
            if from.exists() {
                move_file(&from, &with_ext(&target, ext)).map_err(|e| e.to_string())?;
            }
        }
        "moved"
    };

    crate::config::write_db_path(&app, &target).map_err(|e| e.to_string())?;

    // Reopen at the target so the running app stays consistent until relaunch.
    {
        let mut s = store.lock().map_err(|e| e.to_string())?;
        s.conn = rusqlite::Connection::open(&target).map_err(|e| e.to_string())?;
        crate::migrate::run_migrations(&s.conn).map_err(|e| e.to_string())?;
    }

    // Keep the active context's registry entry pointing at the new path.
    if let Some(reg) = app.try_state::<Mutex<crate::profiles::Registry>>() {
        if let Ok(mut r) = reg.lock() {
            let active = r.active_id.clone();
            if let Some(c) = r.contexts.iter_mut().find(|c| c.id == active) {
                c.path = target.to_string_lossy().into_owned();
            }
            let _ = crate::profiles::save(&crate::config::profiles_path(&app), &r);
        }
    }

    Ok(DbLocationResult {
        mode: mode.to_string(),
        path: target.to_string_lossy().into_owned(),
    })
}

#[tauri::command]
pub fn note_revisions(
    store: State<'_, Mutex<Store>>,
    note_id: String,
) -> Result<Vec<crate::revisions::Revision>, String> {
    let store = store.lock().map_err(|e| e.to_string())?;
    crate::revisions::list_revisions(&store.conn, &note_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn note_revision_content(
    store: State<'_, Mutex<Store>>,
    id: i64,
) -> Result<Option<String>, String> {
    let store = store.lock().map_err(|e| e.to_string())?;
    crate::revisions::revision_content(&store.conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn folder_set_sort(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    id: String,
    sort: String,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        crate::folders::set_folder_sort(&store.conn, &id, &sort).map_err(|e| e.to_string())?;
        if store.sync_enabled {
            crate::folders::touch_folder(&store.conn, &id).map_err(|e| e.to_string())?;
        }
    }
    notify(&app, &webview);
    Ok(())
}

#[tauri::command]
pub async fn mcp_apply_config(
    app: AppHandle,
    enabled: bool,
    bind: String,
    port: u16,
    token: String,
    auth_required: bool,
    allow_write: bool,
) -> Result<(), String> {
    crate::mcp::apply(app, enabled, bind, port, token, auth_required, allow_write).await
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn hide_main(app: AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.hide();
    }
}

#[tauri::command]
pub fn save_image(
    app: AppHandle,
    note_id: String,
    name: String,
    bytes: Vec<u8>,
) -> Result<String, String> {
    let name = crate::images::safe_subpath(&name).ok_or_else(|| "invalid name".to_string())?;
    let sub = crate::images::safe_subpath(&crate::images::shard(&note_id))
        .ok_or_else(|| "invalid note id".to_string())?;
    let dir = crate::images::images_dir(&app).join(&sub);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(&name), &bytes).map_err(|e| e.to_string())?;
    Ok(crate::images::note_image_url(&note_id, &name))
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathChecks {
    db_writable: bool,
    images_writable: bool,
    db_path: String,
    images_path: String,
}

#[tauri::command]
pub fn check_paths(app: AppHandle) -> PathChecks {
    let db = crate::config::read_db_path(&app);
    let db_dir = db.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let images = crate::images::images_dir(&app);
    PathChecks {
        db_writable: crate::syscheck::is_writable(&db_dir),
        images_writable: crate::syscheck::is_writable(&images),
        db_path: db_dir.to_string_lossy().to_string(),
        images_path: images.to_string_lossy().to_string(),
    }
}

fn select_notes(notes: Vec<crate::storage::Note>, ids: &[String]) -> Vec<crate::storage::Note> {
    if ids.is_empty() {
        notes
    } else {
        notes.into_iter().filter(|n| ids.contains(&n.id)).collect()
    }
}

#[tauri::command]
pub fn export_notes_base64(
    store: State<'_, Mutex<Store>>,
    app: AppHandle,
    path: String,
    ids: Vec<String>,
) -> Result<(), String> {
    let notes = {
        let s = store.lock().map_err(|e| e.to_string())?;
        s.load_notes().map_err(|e| e.to_string())?
    };
    let root = crate::images::images_dir(&app);
    let out: Vec<crate::storage::Note> = select_notes(notes, &ids)
        .into_iter()
        .map(|mut n| {
            n.content = crate::export::inline_images(&n.content, |rel| {
                let safe = crate::images::safe_subpath(rel)?;
                let bytes = std::fs::read(root.join(&safe)).ok()?;
                Some((crate::images::mime_for(rel).to_string(), bytes))
            });
            n
        })
        .collect();
    let json = serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn note_inlined_html(
    store: State<'_, Mutex<Store>>,
    app: AppHandle,
    note_id: String,
) -> Result<String, String> {
    let notes = {
        let s = store.lock().map_err(|e| e.to_string())?;
        s.load_all_notes().map_err(|e| e.to_string())?
    };
    let note = notes
        .into_iter()
        .find(|n| n.id == note_id)
        .ok_or_else(|| "note not found".to_string())?;
    let root = crate::images::images_dir(&app);
    Ok(crate::export::inline_images(&note.content, |rel| {
        let safe = crate::images::safe_subpath(rel)?;
        let bytes = std::fs::read(root.join(&safe)).ok()?;
        Some((crate::images::mime_for(rel).to_string(), bytes))
    }))
}

#[tauri::command]
pub fn save_export(path: String, bytes: Vec<u8>) -> Result<(), String> {
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn export_md_bundle(
    app: AppHandle,
    dir: String,
    md: String,
    name: String,
) -> Result<(), String> {
    let (rewritten, paths) = crate::export::to_bundle(&md);
    let root = crate::images::images_dir(&app);
    let dest = std::path::PathBuf::from(&dir);
    std::fs::create_dir_all(dest.join("images")).map_err(|e| e.to_string())?;
    for rel in paths {
        if let Some(safe) = crate::images::safe_subpath(&rel) {
            let to = dest.join("images").join(&safe);
            if let Some(p) = to.parent() {
                let _ = std::fs::create_dir_all(p);
            }
            let _ = std::fs::copy(root.join(&safe), &to);
        }
    }
    let fname = format!("{}.md", name.replace(['/', '\\', ':'], "-"));
    std::fs::write(dest.join(fname), rewritten).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn export_notes_bundle(
    store: State<'_, Mutex<Store>>,
    app: AppHandle,
    dir: String,
    ids: Vec<String>,
) -> Result<(), String> {
    let notes = {
        let s = store.lock().map_err(|e| e.to_string())?;
        s.load_notes().map_err(|e| e.to_string())?
    };
    let root = crate::images::images_dir(&app);
    let dest = std::path::PathBuf::from(&dir);
    std::fs::create_dir_all(dest.join("images")).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for mut n in select_notes(notes, &ids) {
        let (content, paths) = crate::export::to_bundle(&n.content);
        for rel in paths {
            if let Some(safe) = crate::images::safe_subpath(&rel) {
                let to = dest.join("images").join(&safe);
                if let Some(parent) = to.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::copy(root.join(&safe), &to);
            }
        }
        n.content = content;
        out.push(n);
    }
    let json = serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?;
    std::fs::write(dest.join("notes.json"), json).map_err(|e| e.to_string())
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub state: String, // "local" | "unbound" | "syncing" | "synced" | "offline"
    pub last_synced_at: i64,
    pub pending: i64,
}

fn active_server(reg: &crate::profiles::Registry) -> Option<crate::profiles::ContextEntry> {
    reg.active().filter(|c| c.kind == "server").cloned()
}

#[tauri::command]
pub async fn server_workspaces(
    reg: State<'_, Mutex<crate::profiles::Registry>>,
) -> Result<Vec<crate::sync::WorkspaceInfo>, String> {
    let ctx = {
        let r = reg.lock().map_err(|e| e.to_string())?;
        active_server(&r).ok_or("no active server context")?
    };
    let tokens = crate::auth::load_tokens(&ctx.id)?.ok_or("not authenticated")?;
    crate::sync::fetch_workspaces(&ctx.server_url, &tokens.access_token)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn context_bind_workspace(
    app: AppHandle,
    reg: State<'_, Mutex<crate::profiles::Registry>>,
    notify: State<'_, std::sync::Arc<tokio::sync::Notify>>,
    id: String,
    workspace_id: String,
    label: String,
) -> Result<Vec<ContextInfo>, String> {
    let infos = {
        let mut r = reg.lock().map_err(|e| e.to_string())?;
        r.bind_workspace(&id, workspace_id)?;
        if !label.is_empty() {
            r.rename(&id, label)?;
        }
        crate::profiles::save(&crate::config::profiles_path(&app), &r)
            .map_err(|e| e.to_string())?;
        to_infos(&r)
    };
    notify.notify_one(); // kick an immediate sync of the freshly-bound context
    Ok(infos)
}

#[tauri::command]
pub fn sync_now(notify: State<'_, std::sync::Arc<tokio::sync::Notify>>) -> Result<(), String> {
    notify.notify_one();
    Ok(())
}

#[tauri::command]
pub fn sync_status(
    reg: State<'_, Mutex<crate::profiles::Registry>>,
    store: State<'_, Mutex<Store>>,
) -> Result<SyncStatus, String> {
    let r = reg.lock().map_err(|e| e.to_string())?;
    let Some(ctx) = active_server(&r) else {
        return Ok(SyncStatus {
            state: "local".into(),
            last_synced_at: 0,
            pending: 0,
        });
    };
    if ctx.workspace_id.is_empty() {
        return Ok(SyncStatus {
            state: "unbound".into(),
            last_synced_at: 0,
            pending: 0,
        });
    }
    let s = store.lock().map_err(|e| e.to_string())?;
    let last = crate::migrate::get_meta_i64(&s.conn, "sync_last_at", 0);
    let pending = s.load_dirty_notes().map_err(|e| e.to_string())?.len() as i64
        + crate::folders::load_dirty_folders(&s.conn)
            .map_err(|e| e.to_string())?
            .len() as i64;
    let state = if last > 0 { "synced" } else { "syncing" };
    Ok(SyncStatus {
        state: state.into(),
        last_synced_at: last,
        pending,
    })
}

/// One push-then-pull cycle for the active server context. Locks are released
/// before every network `.await`. No-op for local/unbound/unauthenticated.
pub async fn run_sync_cycle(app: &AppHandle) -> Result<(), String> {
    let reg_state = app.state::<Mutex<crate::profiles::Registry>>();
    let store_state = app.state::<Mutex<Store>>();

    let ctx = {
        let r = reg_state.lock().map_err(|e| e.to_string())?;
        active_server(&r)
    };
    let Some(ctx) = ctx else {
        return Ok(());
    };
    if ctx.workspace_id.is_empty() {
        return Ok(());
    }
    let Some(tokens) = crate::auth::load_tokens(&ctx.id)? else {
        return Ok(());
    };

    let _ = app.emit(
        "sync-status",
        SyncStatus {
            state: "syncing".into(),
            last_synced_at: 0,
            pending: 0,
        },
    );

    // Collect dirty rows + cursor under the lock, then release it.
    let (folders, notes, note_ids, folder_ids, since) = {
        let s = store_state.lock().map_err(|e| e.to_string())?;
        let dn = s.load_dirty_notes().map_err(|e| e.to_string())?;
        let df = crate::folders::load_dirty_folders(&s.conn).map_err(|e| e.to_string())?;
        let since = crate::migrate::get_meta_i64(&s.conn, "sync_cursor", 0);
        let folders: Vec<_> = df.iter().map(crate::sync::folder_to_wire).collect();
        let notes: Vec<_> = dn.iter().map(crate::sync::note_to_wire).collect();
        // Snapshot (id, updated_at) so the post-sync dirty-clear skips any row
        // re-edited during the network window (its updated_at will have changed).
        let note_ids: Vec<(String, i64)> =
            dn.iter().map(|n| (n.id.clone(), n.updated_at)).collect();
        let folder_ids: Vec<(String, i64)> =
            df.iter().map(|f| (f.id.clone(), f.updated_at)).collect();
        (folders, notes, note_ids, folder_ids, since)
    };

    // Network: push then pull (no lock held).
    let result = async {
        crate::sync::push(
            &ctx.server_url,
            &tokens.access_token,
            &ctx.workspace_id,
            folders,
            notes,
        )
        .await?;
        crate::sync::pull(
            &ctx.server_url,
            &tokens.access_token,
            &ctx.workspace_id,
            since,
        )
        .await
    }
    .await;

    match result {
        Ok((cursor, pf, pn)) => {
            {
                let s = store_state.lock().map_err(|e| e.to_string())?;
                s.clear_note_dirty(&note_ids).map_err(|e| e.to_string())?;
                crate::folders::clear_folder_dirty(&s.conn, &folder_ids)
                    .map_err(|e| e.to_string())?;
                crate::sync::apply_pulled(&s, &pf, &pn).map_err(|e| e.to_string())?;
                crate::migrate::set_meta_i64(&s.conn, "sync_cursor", cursor)
                    .map_err(|e| e.to_string())?;
                crate::migrate::set_meta_i64(&s.conn, "sync_last_at", now_ms())
                    .map_err(|e| e.to_string())?;
            }
            broadcast_context_changed(app); // refresh the UI from the updated cache
                                            // S2b: transfer referenced images (non-fatal — notes already synced).
            let _ = run_image_phase(app, &ctx, &tokens.access_token).await;
            let _ = app.emit(
                "sync-status",
                SyncStatus {
                    state: "synced".into(),
                    last_synced_at: now_ms(),
                    pending: 0,
                },
            );
            Ok(())
        }
        Err(e) => {
            let _ = app.emit(
                "sync-status",
                SyncStatus {
                    state: "offline".into(),
                    last_synced_at: 0,
                    pending: 0,
                },
            );
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub fn notes_load_all(
    reg: State<'_, Mutex<crate::profiles::Registry>>,
) -> Result<Vec<crate::aggregate::TaggedMeta>, String> {
    let contexts = registry_contexts(&reg)?;
    Ok(crate::aggregate::aggregate_meta(&contexts))
}

/// Combined-view search: full-text across every context, tagged with context.
/// The vault is a single local vault shared by every context, so the same
/// lock state gates protected rows everywhere.
#[tauri::command]
pub fn notes_search_all(
    reg: State<'_, Mutex<crate::profiles::Registry>>,
    vault: VaultStateHandle<'_>,
    query: String,
) -> Result<Vec<crate::aggregate::TaggedHit>, String> {
    let exclude_protected = !vault.lock().map_err(|e| e.to_string())?.is_unlocked();
    let contexts = registry_contexts(&reg)?;
    Ok(crate::aggregate::search_all(
        &contexts,
        &query,
        50,
        exclude_protected,
    ))
}

/// Snapshot the registry's contexts as aggregator `Ctx` descriptors.
fn registry_contexts(
    reg: &State<'_, Mutex<crate::profiles::Registry>>,
) -> Result<Vec<crate::aggregate::Ctx>, String> {
    let r = reg.lock().map_err(|e| e.to_string())?;
    Ok(r.contexts
        .iter()
        .map(|c| crate::aggregate::Ctx {
            id: c.id.clone(),
            label: c.label.clone(),
            kind: c.kind.clone(),
            path: c.path.clone(),
        })
        .collect())
}

/// S2b: after the note phase, transfer referenced image blobs for a bound server
/// context. Non-fatal — any failure is logged and retried next cycle; the note
/// sync is already committed. No Store lock is held across a network `.await`.
pub async fn run_image_phase(
    app: &AppHandle,
    ctx: &crate::profiles::ContextEntry,
    token: &str,
) -> Result<(), String> {
    use std::collections::HashSet;

    let store_state = app.state::<Mutex<Store>>();
    let images_root = crate::images::images_dir(app);

    // Referenced relpaths (under lock), then which of them exist locally.
    let referenced: HashSet<String> = {
        let s = store_state.lock().map_err(|e| e.to_string())?;
        crate::images::collect_referenced(&s)
    };
    let local_present: HashSet<String> = referenced
        .iter()
        .filter(|p| {
            crate::images::safe_subpath(p)
                .map(|sp| images_root.join(sp).is_file())
                .unwrap_or(false)
        })
        .cloned()
        .collect();

    // Manifest (network). On failure: skip the image phase (offline / not ready).
    let server =
        crate::imagesync::fetch_manifest(&ctx.server_url, token, &ctx.workspace_id).await?;

    // Upload local-only referenced images.
    for path in crate::imagesync::to_upload(&local_present, &server) {
        let Some(sp) = crate::images::safe_subpath(&path) else {
            continue;
        };
        let file = images_root.join(&sp);
        if let Ok(bytes) = std::fs::read(&file) {
            let mime = crate::images::mime_for(&path);
            if let Err(e) = crate::imagesync::upload_image(
                &ctx.server_url,
                token,
                &ctx.workspace_id,
                &path,
                bytes,
                mime,
            )
            .await
            {
                eprintln!("image upload {path} failed: {e}");
            }
        }
    }

    // Download referenced images we lack locally.
    for path in crate::imagesync::to_download(&referenced, &local_present, &server) {
        let Some(sp) = crate::images::safe_subpath(&path) else {
            continue;
        };
        match crate::imagesync::download_image(&ctx.server_url, token, &ctx.workspace_id, &path)
            .await
        {
            Ok(bytes) => {
                let dest = images_root.join(&sp);
                if let Some(parent) = dest.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::write(&dest, bytes);
            }
            Err(e) => eprintln!("image download {path} failed: {e}"),
        }
    }
    Ok(())
}

// Protected-notes vault commands (Task 5): status/setup/unlock/lock and
// passphrase change. The unlocked DEK lives only in the managed
// `VaultState` — it is never logged, persisted, or returned to the
// frontend. Only `vault_setup` returns secret material, and only the
// recovery key (shown once, by design).

type VaultStateHandle<'r> = State<'r, Mutex<crate::vault::state::VaultState>>;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub exists: bool,
    pub unlocked: bool,
    pub biometric: bool,
}

/// Loads and parses the persisted vault record, or a "not set up" error if
/// none exists yet.
fn load_vault_record(store: &State<'_, Mutex<Store>>) -> Result<crate::vault::VaultRecord, String> {
    let json = {
        let store = store.lock().map_err(|e| e.to_string())?;
        store.vault_record().map_err(|e| e.to_string())?
    };
    let json = json.ok_or_else(|| "vault: not set up".to_string())?;
    crate::vault::VaultRecord::from_json(&json).map_err(String::from)
}

#[tauri::command]
pub fn vault_status(
    store: State<'_, Mutex<Store>>,
    vault: VaultStateHandle<'_>,
) -> Result<VaultStatus, String> {
    let exists = {
        let store = store.lock().map_err(|e| e.to_string())?;
        store.vault_record().map_err(|e| e.to_string())?.is_some()
    };
    let unlocked = vault.lock().map_err(|e| e.to_string())?.is_unlocked();
    Ok(VaultStatus {
        exists,
        unlocked,
        // Biometric unlock is offered only when the device can evaluate Touch
        // ID (`is_available`) AND the user has enrolled a wrapped DEK
        // (`is_enrolled`). `is_enrolled` reads the keychain without prompting.
        biometric: crate::vault::biometric::is_available()
            && crate::vault::biometric::is_enrolled(),
    })
}

/// Whether this device can evaluate biometric authentication (macOS Touch ID).
/// `false` on non-macOS desktop and on mobile.
#[tauri::command]
pub fn vault_biometric_available() -> bool {
    crate::vault::biometric::is_available()
}

/// Enrolls biometric unlock: stores the currently-unlocked DEK in the keychain
/// so it can later be released after a Touch ID prompt. Requires the vault to
/// be unlocked — the DEK is taken from the live `VaultState`, never re-derived.
#[tauri::command]
pub fn vault_biometric_enable(vault: VaultStateHandle<'_>) -> Result<(), String> {
    let vault = vault.lock().map_err(|e| e.to_string())?;
    let dek = vault.dek().ok_or_else(|| "vault locked".to_string())?;
    crate::vault::biometric::store_dek(dek).map_err(String::from)
}

/// Disables biometric unlock by deleting the keychain-stored DEK. Idempotent.
#[tauri::command]
pub fn vault_biometric_disable() -> Result<(), String> {
    crate::vault::biometric::clear().map_err(String::from)
}

/// Unlocks the vault via biometrics: prompt Touch ID, then release the
/// keychain-wrapped DEK into `VaultState`. Async so the blocking Touch ID
/// prompt runs off the main thread (`spawn_blocking`) — otherwise the main run
/// loop would be blocked and could not present the system dialog.
#[tauri::command]
pub async fn vault_unlock_biometric(vault: VaultStateHandle<'_>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(|| {
        crate::vault::biometric::authenticate("Unlock your protected notes")
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(String::from)?;
    let dek = crate::vault::biometric::load_dek()
        .map_err(String::from)?
        .ok_or_else(|| "vault: biometric unlock is not set up".to_string())?;
    vault.lock().map_err(|e| e.to_string())?.unlock(dek);
    Ok(())
}

/// Creates a new vault: wraps a fresh DEK under `passphrase`, persists the
/// record, and unlocks it immediately. Returns the one-time recovery key
/// split into its dash-separated groups — the only place it is ever exposed.
#[tauri::command]
pub fn vault_setup(
    store: State<'_, Mutex<Store>>,
    vault: VaultStateHandle<'_>,
    passphrase: String,
) -> Result<Vec<String>, String> {
    // Guard against clobbering an existing vault: a second `vault_setup` call
    // would generate a brand-new DEK and overwrite the stored record,
    // orphaning the old DEK and permanently losing any notes already
    // encrypted under it. This is a Tauri command (the trust boundary), so
    // the check has to live here rather than relying on the frontend to gate
    // it. Nothing is generated or persisted until we know no record exists.
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        if store.vault_record().map_err(|e| e.to_string())?.is_some() {
            return Err("vault: a vault already exists".to_string());
        }
    }
    let (record, recovery_key, dek) = crate::vault::setup(&passphrase).map_err(String::from)?;
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        store
            .set_vault_record(&record.to_json())
            .map_err(|e| e.to_string())?;
    }
    vault.lock().map_err(|e| e.to_string())?.unlock(dek);
    Ok(recovery_key.as_str().split('-').map(String::from).collect())
}

#[tauri::command]
pub fn vault_unlock(
    store: State<'_, Mutex<Store>>,
    vault: VaultStateHandle<'_>,
    passphrase: String,
) -> Result<(), String> {
    let record = load_vault_record(&store)?;
    let dek = crate::vault::unlock_passphrase(&record, &passphrase).map_err(String::from)?;
    vault.lock().map_err(|e| e.to_string())?.unlock(dek);
    Ok(())
}

#[tauri::command]
pub fn vault_unlock_recovery(
    store: State<'_, Mutex<Store>>,
    vault: VaultStateHandle<'_>,
    recovery: String,
) -> Result<(), String> {
    let record = load_vault_record(&store)?;
    let dek = crate::vault::unlock_recovery(&record, &recovery).map_err(String::from)?;
    vault.lock().map_err(|e| e.to_string())?.unlock(dek);
    Ok(())
}

#[tauri::command]
pub fn vault_lock(vault: VaultStateHandle<'_>) -> Result<(), String> {
    vault.lock().map_err(|e| e.to_string())?.lock();
    Ok(())
}

/// Unlocks with `current`, re-wraps the same DEK under `next`, persists the
/// updated record, and leaves the vault unlocked (existing recovery key
/// keeps working — `rewrap_passphrase` never touches it).
///
/// On success this re-unlocks `VaultState` by design: `current` was just
/// cryptographically re-verified via `unlock_passphrase`, so re-arming the
/// session with the (unchanged) DEK is safe rather than forcing a redundant
/// unlock.
#[tauri::command]
pub fn vault_change_passphrase(
    store: State<'_, Mutex<Store>>,
    vault: VaultStateHandle<'_>,
    current: String,
    next: String,
) -> Result<(), String> {
    let record = load_vault_record(&store)?;
    let dek = crate::vault::unlock_passphrase(&record, &current).map_err(String::from)?;
    let new_record = crate::vault::rewrap_passphrase(&record, &dek, &next);
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        store
            .set_vault_record(&new_record.to_json())
            .map_err(|e| e.to_string())?;
    }
    vault.lock().map_err(|e| e.to_string())?.unlock(dek);
    Ok(())
}

/// Encrypts or decrypts one note's stored content in place, keeping
/// `notes.protected` in sync with the physical content state. Requires the
/// vault to be unlocked.
///
/// `protected = false` is refused with an error while the note is inside a
/// `locked` folder — the folder is the source of truth for that note's
/// protection until the folder itself is unlocked.
///
/// Transitioning to `protected = true` discards the note's existing revision
/// history (see `crate::revisions::delete_revisions`) — v1 behavior, since
/// `note_revisions` is unencrypted.
#[tauri::command]
pub fn note_set_protected(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    vault: VaultStateHandle<'_>,
    id: String,
    protected: bool,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        let vault = vault.lock().map_err(|e| e.to_string())?;
        let dek = vault.dek().ok_or_else(|| "vault locked".to_string())?;

        if protected {
            if !store.note_protected(&id).map_err(|e| e.to_string())? {
                // Seal + flip `protected` + mark dirty + purge the plaintext
                // revision history (v1: keeping it would defeat
                // encryption-at-rest, since note_revisions is unencrypted).
                encrypt_note_in_place(&store, &id, dek)?;
            }
        } else {
            if has_locked_ancestor_folder(&store, &id)? {
                return Err("note is protected by its folder".to_string());
            }
            if store.note_protected(&id).map_err(|e| e.to_string())? {
                let ciphertext = store
                    .load_note_content(&id)
                    .map_err(|e| e.to_string())?
                    .unwrap_or_default();
                let plaintext = open_content(dek, &id, &ciphertext)?;
                store
                    .set_content_silent(&id, &plaintext)
                    .map_err(|e| e.to_string())?;
                store
                    .set_note_protected(&id, false)
                    .map_err(|e| e.to_string())?;
            }
        }
    }
    notify(&app, &webview);
    Ok(())
}

/// Locks or unlocks a folder, encrypting/decrypting the notes in its subtree
/// to match. Requires the vault to be unlocked.
///
/// v1 limitation: `notes.protected` tracks only physical ciphertext state,
/// not a separate "individually locked" intent, so unlocking a folder
/// decrypts every subtree note that has no *other* locked ancestor —
/// including a note that was individually protected while it happened to
/// live inside this now-unlocking folder. Acceptable for v1.
///
/// Locking (not unlocking) also discards each newly-encrypted note's
/// existing revision history, same rationale as `note_set_protected`.
#[tauri::command]
pub fn folder_set_locked(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    vault: VaultStateHandle<'_>,
    id: String,
    locked: bool,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        let vault = vault.lock().map_err(|e| e.to_string())?;
        let dek = vault.dek().ok_or_else(|| "vault locked".to_string())?;
        let note_ids = store.note_ids_in_subtree(&id).map_err(|e| e.to_string())?;

        if locked {
            store
                .set_folder_locked(&id, true)
                .map_err(|e| e.to_string())?;
            for note_id in &note_ids {
                if !store.note_protected(note_id).map_err(|e| e.to_string())? {
                    // Same transition as note_set_protected(id, true): seal +
                    // flip `protected` + mark dirty + discard this note's now
                    // encryption-defeating plaintext revision history.
                    encrypt_note_in_place(&store, note_id, dek)?;
                }
            }
        } else {
            store
                .set_folder_locked(&id, false)
                .map_err(|e| e.to_string())?;
            for note_id in &note_ids {
                let still_locked = has_locked_ancestor_folder(&store, note_id)?;
                if store.note_protected(note_id).map_err(|e| e.to_string())? && !still_locked {
                    let ciphertext = store
                        .load_note_content(note_id)
                        .map_err(|e| e.to_string())?
                        .unwrap_or_default();
                    let plaintext = open_content(dek, note_id, &ciphertext)?;
                    store
                        .set_content_silent(note_id, &plaintext)
                        .map_err(|e| e.to_string())?;
                    store
                        .set_note_protected(note_id, false)
                        .map_err(|e| e.to_string())?;
                }
            }
        }
    }
    notify(&app, &webview);
    Ok(())
}

#[cfg(test)]
mod widget_url_tests {
    use super::{parse_widget_url, WidgetAction};

    #[test]
    fn new_note_url() {
        assert_eq!(parse_widget_url("notefix://new"), WidgetAction::NewNote);
        assert_eq!(parse_widget_url("notefix://new/"), WidgetAction::NewNote);
    }

    #[test]
    fn open_note_url() {
        assert_eq!(
            parse_widget_url("notefix://note/abc-123"),
            WidgetAction::OpenNote("abc-123".into())
        );
    }

    #[test]
    fn auth_and_junk_fall_back_to_auth() {
        assert_eq!(
            parse_widget_url("notefix://auth?code=x"),
            WidgetAction::Auth
        );
        assert_eq!(parse_widget_url("notefix://note"), WidgetAction::Auth); // no id
        assert_eq!(parse_widget_url("garbage"), WidgetAction::Auth);
    }
}

// Task 6 has no Tauri `State` mocking harness available (same constraint
// noted by Task 5), so command wiring is verified by reading `notes_load_one`
// / `notes_save` / `note_set_protected` / `folder_set_locked` end-to-end
// rather than by invoking the `#[tauri::command]` functions directly. What
// IS unit-testable without a harness — the `seal_content`/`open_content`
// helpers, and the `Store`-level effect a protected save has on `content` —
// is covered here.
#[cfg(test)]
mod protected_content_tests {
    use super::*;
    use crate::storage::{Note, Store};
    use crate::vault::aead::Dek;

    #[test]
    fn seal_open_content_roundtrip() {
        let dek = Dek::random();
        let stored = seal_content(&dek, "n1", "<p>secret</p>");
        assert_ne!(stored, "<p>secret</p>");
        assert_eq!(open_content(&dek, "n1", &stored).unwrap(), "<p>secret</p>");
    }

    #[test]
    fn open_content_rejects_mismatched_note_id() {
        // The note id is bound in as AEAD associated data, so a sealed blob
        // can't be silently opened under a different note's id.
        let dek = Dek::random();
        let stored = seal_content(&dek, "n1", "<p>secret</p>");
        assert!(open_content(&dek, "other-note", &stored).is_err());
    }

    #[test]
    fn open_content_rejects_wrong_key() {
        let stored = seal_content(&Dek::random(), "n1", "<p>secret</p>");
        assert!(open_content(&Dek::random(), "n1", &stored).is_err());
    }

    /// Mirrors the branch `notes_save` takes when
    /// `store.is_effectively_protected(id)` is true: seal the plaintext,
    /// persist it, and flip `protected`. No Tauri `State` harness exists in
    /// this repo, so this exercises `Store` + the helpers directly instead of
    /// invoking the `#[tauri::command]` fn.
    #[test]
    fn protected_save_stores_ciphertext_and_decrypts_back() {
        let s = Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        let dek = Dek::random();
        let plaintext = "<p>very secret</p>";

        s.save_note(&Note {
            id: "n1".into(),
            content: plaintext.into(),
            updated_at: 1,
            ..Default::default()
        })
        .unwrap();
        assert!(!s.is_effectively_protected("n1").unwrap());

        s.set_note_protected("n1", true).unwrap();
        assert!(s.is_effectively_protected("n1").unwrap());

        // What notes_save does once is_effectively_protected(id) is true.
        let sealed = seal_content(&dek, "n1", plaintext);
        s.set_content_silent("n1", &sealed).unwrap();

        let stored = s.load_note_content("n1").unwrap().unwrap();
        assert_ne!(stored, plaintext);
        assert!(!stored.contains("very secret"));

        assert_eq!(open_content(&dek, "n1", &stored).unwrap(), plaintext);
    }

    /// Mirrors the encrypt branch `note_set_protected(id, true)` takes: seal,
    /// persist, flip `protected`, then purge revision history. Proves the
    /// purge actually empties `note_revisions` for the protected note while
    /// leaving an untouched, unprotected note's history intact.
    #[test]
    fn protecting_a_note_purges_its_revision_history() {
        let s = Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        let dek = Dek::random();

        s.save_note(&Note {
            id: "a".into(),
            content: "<p>v1</p>".into(),
            updated_at: 1,
            ..Default::default()
        })
        .unwrap();
        crate::revisions::add_revision(&s.conn, "a", "<p>v1</p>", 50).unwrap();

        s.save_note(&Note {
            id: "b".into(),
            content: "<p>v1</p>".into(),
            updated_at: 1,
            ..Default::default()
        })
        .unwrap();
        crate::revisions::add_revision(&s.conn, "b", "<p>v1</p>", 50).unwrap();

        assert_eq!(
            crate::revisions::list_revisions(&s.conn, "a")
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            crate::revisions::list_revisions(&s.conn, "b")
                .unwrap()
                .len(),
            1
        );

        // What note_set_protected(id, true) does for note "a".
        let plaintext = s.load_note_content("a").unwrap().unwrap();
        let sealed = seal_content(&dek, "a", &plaintext);
        s.set_content_silent("a", &sealed).unwrap();
        s.set_note_protected("a", true).unwrap();
        crate::revisions::delete_revisions(&s.conn, "a").unwrap();

        assert!(crate::revisions::list_revisions(&s.conn, "a")
            .unwrap()
            .is_empty());
        // "b" was never protected, so its history is untouched.
        assert_eq!(
            crate::revisions::list_revisions(&s.conn, "b")
                .unwrap()
                .len(),
            1
        );
    }

    /// I1: encrypting a note on a protect/lock transition must leave its row
    /// dirty and `updated_at` bumped, so the freshly-sealed ciphertext +
    /// `protected = 1` are pushed — instead of the server retaining the
    /// pre-protection plaintext (and a later resync clobbering local ciphertext
    /// back to plaintext under the LWW guard). Exercised through
    /// `encrypt_note_in_place`, the shared primitive every encrypt path runs
    /// (`note_set_protected(true)` / `folder_set_locked(true)` / move / reorder).
    #[test]
    fn encrypting_a_note_marks_it_dirty_for_push_when_syncing() {
        let mut s = Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        s.sync_enabled = true;
        let dek = Dek::random();

        s.save_note(&Note {
            id: "n1".into(),
            content: "<p>secret</p>".into(),
            updated_at: 1,
            ..Default::default()
        })
        .unwrap();
        // A sync-enabled save is already dirty; clear it first so the test
        // proves the *protect transition* itself re-dirties the row.
        let ts = s.load_dirty_notes().unwrap()[0].updated_at;
        s.clear_note_dirty(&[("n1".into(), ts)]).unwrap();
        assert!(s.load_dirty_notes().unwrap().is_empty());

        encrypt_note_in_place(&s, "n1", &dek).unwrap();

        let dirty = s.load_dirty_notes().unwrap();
        assert_eq!(dirty.len(), 1);
        assert_eq!(dirty[0].id, "n1");
        assert!(
            dirty[0].protected,
            "the pushed row must carry protected = 1"
        );
        // What propagates is ciphertext, never the plaintext the server must
        // not keep.
        assert_ne!(dirty[0].content, "<p>secret</p>");
        assert!(!dirty[0].content.contains("secret"));
        assert!(dirty[0].updated_at >= ts);
    }

    /// (a) A plaintext note with an existing revision, moved into a locked
    /// folder while the vault is unlocked: `reconcile_folder_move` encrypts
    /// it in place (stored content != plaintext, `note_protected` flips to
    /// true) and purges its revision history.
    #[test]
    fn reconcile_folder_move_encrypts_plaintext_note_into_locked_folder() {
        let s = Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        crate::folders::create_folder(&s.conn, "locked-folder", "Locked", None).unwrap();
        s.set_folder_locked("locked-folder", true).unwrap();
        let dek = Dek::random();
        let plaintext = "<p>very secret</p>";

        s.save_note(&Note {
            id: "n1".into(),
            content: plaintext.into(),
            updated_at: 1,
            ..Default::default()
        })
        .unwrap();
        crate::revisions::add_revision(&s.conn, "n1", plaintext, 50).unwrap();
        assert_eq!(
            crate::revisions::list_revisions(&s.conn, "n1")
                .unwrap()
                .len(),
            1
        );

        reconcile_folder_move(&s, "n1", Some("locked-folder"), Some(&dek)).unwrap();

        assert_eq!(
            s.load_notes().unwrap()[0].folder_id.as_deref(),
            Some("locked-folder")
        );
        assert!(s.note_protected("n1").unwrap());
        let stored = s.load_note_content("n1").unwrap().unwrap();
        assert_ne!(stored, plaintext);
        assert!(!stored.contains("very secret"));
        assert_eq!(open_content(&dek, "n1", &stored).unwrap(), plaintext);
        assert!(crate::revisions::list_revisions(&s.conn, "n1")
            .unwrap()
            .is_empty());
    }

    /// (b) Same setup, but the vault is locked (`dek: None`):
    /// `reconcile_folder_move` refuses with `Err("vault locked")` and
    /// performs no partial move — the note stays in its original folder,
    /// stays plaintext, and keeps its revision — no leak, no half-applied
    /// state.
    #[test]
    fn reconcile_folder_move_refuses_when_vault_locked_for_locked_destination() {
        let s = Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        crate::folders::create_folder(&s.conn, "locked-folder", "Locked", None).unwrap();
        s.set_folder_locked("locked-folder", true).unwrap();
        let plaintext = "<p>very secret</p>";

        s.save_note(&Note {
            id: "n1".into(),
            content: plaintext.into(),
            updated_at: 1,
            ..Default::default()
        })
        .unwrap();
        crate::revisions::add_revision(&s.conn, "n1", plaintext, 50).unwrap();

        let err = reconcile_folder_move(&s, "n1", Some("locked-folder"), None).unwrap_err();
        assert_eq!(err, "vault locked");

        // Unmoved, still plaintext, revision untouched.
        assert_eq!(s.load_notes().unwrap()[0].folder_id, None);
        assert!(!s.note_protected("n1").unwrap());
        assert_eq!(
            s.load_note_content("n1").unwrap().as_deref(),
            Some(plaintext)
        );
        assert_eq!(
            crate::revisions::list_revisions(&s.conn, "n1")
                .unwrap()
                .len(),
            1
        );
    }

    /// C1 (a): drag/reorder of a plaintext note (with an existing revision)
    /// into a locked folder while the vault is unlocked must be as safe as the
    /// context-menu move path — `reconcile_reorder` repositions AND encrypts it
    /// in place (ciphertext at rest, `protected` flips true) and purges its
    /// plaintext revision history, so DnD can't leave plaintext in a locked
    /// subtree.
    #[test]
    fn reconcile_reorder_encrypts_plaintext_note_into_locked_folder() {
        let s = Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        crate::folders::create_folder(&s.conn, "locked-folder", "Locked", None).unwrap();
        s.set_folder_locked("locked-folder", true).unwrap();
        let dek = Dek::random();
        let plaintext = "<p>very secret</p>";

        s.save_note(&Note {
            id: "n1".into(),
            content: plaintext.into(),
            updated_at: 1,
            ..Default::default()
        })
        .unwrap();
        crate::revisions::add_revision(&s.conn, "n1", plaintext, 50).unwrap();
        assert_eq!(
            crate::revisions::list_revisions(&s.conn, "n1")
                .unwrap()
                .len(),
            1
        );

        reconcile_reorder(&s, Some("locked-folder"), &["n1".to_string()], Some(&dek)).unwrap();

        assert_eq!(
            s.load_notes().unwrap()[0].folder_id.as_deref(),
            Some("locked-folder")
        );
        assert!(s.note_protected("n1").unwrap());
        let stored = s.load_note_content("n1").unwrap().unwrap();
        assert_ne!(stored, plaintext);
        assert!(!stored.contains("very secret"));
        assert_eq!(open_content(&dek, "n1", &stored).unwrap(), plaintext);
        assert!(crate::revisions::list_revisions(&s.conn, "n1")
            .unwrap()
            .is_empty());
    }

    /// C1 (b): the same reorder-into-locked while the vault is LOCKED
    /// (`dek: None`) must refuse the whole operation and mutate nothing — the
    /// note stays at root, stays plaintext, and keeps its revision. No plaintext
    /// is ever stranded inside a locked folder, not even transiently.
    #[test]
    fn reconcile_reorder_refuses_when_vault_locked_for_locked_destination() {
        let s = Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        crate::folders::create_folder(&s.conn, "locked-folder", "Locked", None).unwrap();
        s.set_folder_locked("locked-folder", true).unwrap();
        let plaintext = "<p>very secret</p>";

        s.save_note(&Note {
            id: "n1".into(),
            content: plaintext.into(),
            updated_at: 1,
            ..Default::default()
        })
        .unwrap();
        crate::revisions::add_revision(&s.conn, "n1", plaintext, 50).unwrap();

        let err =
            reconcile_reorder(&s, Some("locked-folder"), &["n1".to_string()], None).unwrap_err();
        assert_eq!(err, "vault locked");

        // Unmoved, still plaintext, revision untouched.
        assert_eq!(s.load_notes().unwrap()[0].folder_id, None);
        assert!(!s.note_protected("n1").unwrap());
        assert_eq!(
            s.load_note_content("n1").unwrap().as_deref(),
            Some(plaintext)
        );
        assert_eq!(
            crate::revisions::list_revisions(&s.conn, "n1")
                .unwrap()
                .len(),
            1
        );
    }
}
