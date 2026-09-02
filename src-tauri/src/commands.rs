//! The Tauri command surface.
//!
//! These functions are deliberately thin: they acquire the managed `State`
//! mutexes, hand plain values to [`crate::ops`] (where the actual logic —
//! validation, branching, store/vault mutations — lives and is unit-tested),
//! and then do the things only Tauri can do: emit events to windows, rebuild
//! the tray menu, run the image GC, open windows.
//!
//! Anything that is pure OS/Tauri plumbing (window/tray control, dialogs, the
//! network `.await` wrappers) stays here in full, since there is nothing
//! testable to extract.

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::ops;
use crate::storage::{Note, NoteMeta, SearchHit, Store};

// Re-exported so `crate::commands::…` keeps naming the same items after the
// logic moved into `ops` — `mcp.rs` reaches for the two crypto primitives, and
// the result types below are part of several command signatures.
pub(crate) use crate::ops::{encrypt_note_in_place, open_content};
pub use crate::ops::{ContextInfo, DbLocationResult, PathChecks, SyncStatus, VaultStatus};

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

fn notify(app: &AppHandle, webview: &WebviewWindow) {
    broadcast_changed(app, webview.label());
    crate::tray::rebuild_menu(app);
}

// ---------------------------------------------------------------------------
// Notes
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn notes_load(store: State<'_, Mutex<Store>>) -> Result<Vec<NoteMeta>, String> {
    let store = store.lock().map_err(|e| e.to_string())?;
    store.load_notes_meta().map_err(|e| e.to_string())
}

/// The full HTML content of one note (empty string if it no longer exists).
/// A protected note requires the vault to hold the DEK it was actually
/// sealed with — `Err("vault locked")` when the ring is empty, `Err("key
/// generation not available")` when it's unlocked but lacks that note's
/// generation — and is decrypted before returning.
#[tauri::command]
pub fn notes_load_one(
    store: State<'_, Mutex<Store>>,
    vault: VaultStateHandle<'_>,
    id: String,
) -> Result<String, String> {
    let store = store.lock().map_err(|e| e.to_string())?;
    // Recover from a poisoned `VaultState` instead of propagating the poison,
    // the way `swap_store_to` does. `VaultState` is a ring of DEKs plus a
    // timestamp, mutated only by whole-field assignments, so a panic elsewhere
    // cannot leave it half-updated. Without this, a single panic anywhere
    // holding that mutex would break reads of UNPROTECTED notes too — this is
    // the one read on the hot path, and its guard is taken unconditionally.
    // The protected path is unchanged: a locked vault still yields
    // `Err("vault locked")` from `ops::open_note_content`.
    let vault = vault
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    ops::open_note_content(&store, &vault, &id)
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
    let unlocked = vault.lock().map_err(|e| e.to_string())?.is_unlocked();
    let store = store.lock().map_err(|e| e.to_string())?;
    ops::search_notes(&store, &query, unlocked)
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
        let vault = vault.lock().map_err(|e| e.to_string())?;
        ops::save_note(&store, vault.dek().zip(vault.newest_generation()), &note)?;
    }
    notify(&app, &webview);
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
        ops::delete_note(&store, &id, now_ms())?;
        crate::images::run_gc(&app, &store);
    }
    notify(&app, &webview);
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
    notify(&app, &webview);
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
    notify(&app, &webview);
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
    notify(&app, &webview);
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
    notify(&app, &webview);
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
    notify(&app, &webview);
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
    notify(&app, &webview);
    Ok(())
}

/// Sets a note's "Hide from MCP" opt-out (schema v14). Plaintext local flag —
/// no vault involved. See `Store::is_effectively_mcp_hidden` for how it's
/// enforced on the MCP surface.
#[tauri::command]
pub fn note_set_mcp_hidden(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    id: String,
    hidden: bool,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        store
            .set_note_mcp_hidden(&id, hidden)
            .map_err(|e| e.to_string())?;
    }
    notify(&app, &webview);
    Ok(())
}

#[tauri::command]
pub fn export_notes(
    store: State<'_, Mutex<Store>>,
    path: String,
    ids: Vec<String>,
) -> Result<(), String> {
    // Load under the lock, then release it before writing to disk.
    let notes = {
        let s = store.lock().map_err(|e| e.to_string())?;
        s.load_notes().map_err(|e| e.to_string())?
    };
    ops::export_notes_json(&notes, std::path::Path::new(&path), &ids)
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
    notify(&app, &webview);
    Ok(())
}

#[tauri::command]
pub fn note_stats(store: State<'_, Mutex<Store>>) -> Result<crate::stats::Stats, String> {
    let store = store.lock().map_err(|e| e.to_string())?;
    ops::note_stats(&store)
}

// ---------------------------------------------------------------------------
// Folders
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn folders_load(store: State<'_, Mutex<Store>>) -> Result<Vec<crate::folders::Folder>, String> {
    let store = store.lock().map_err(|e| e.to_string())?;
    crate::folders::load_folders(&store.conn).map_err(|e| e.to_string())
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
        ops::folder_create(&store, &id, &name, parent_id.as_deref())?;
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
        ops::folder_rename(&store, &id, &name)?;
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
        ops::folder_move(&store, &id, parent_id.as_deref())?;
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
        ops::folder_delete(&store, &id, &mode)?;
        crate::images::run_gc(&app, &store);
    }
    notify(&app, &webview);
    Ok(())
}

/// Moves a note to a (possibly different) folder — see
/// `ops::reconcile_folder_move` for the encrypt-on-move-in policy.
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
        ops::reconcile_folder_move(
            &store,
            &id,
            folder_id.as_deref(),
            vault.dek().zip(vault.newest_generation()),
        )?;
    }
    notify(&app, &webview);
    Ok(())
}

/// Reorders/repositions notes within a folder — see `ops::reconcile_reorder`
/// for the encrypt-on-drop-into-locked policy that keeps drag-and-drop as safe
/// as the context-menu move path (`notes_set_folder`).
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
        ops::reconcile_reorder(
            &store,
            folder_id.as_deref(),
            &ids,
            vault.dek().zip(vault.newest_generation()),
        )?;
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
        ops::folders_reorder(&store, parent_id.as_deref(), &ids)?;
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
        ops::folder_set_icon(&store, &id, &icon)?;
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
        ops::folder_set_color(&store, &id, &color)?;
    }
    notify(&app, &webview);
    Ok(())
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
        ops::folder_set_sort(&store, &id, &sort)?;
    }
    notify(&app, &webview);
    Ok(())
}

/// Sets a folder's "Hide from MCP" opt-out (schema v14) — every note in its
/// subtree becomes effectively hidden too, mirroring how a locked folder
/// protects its subtree (`Store::is_effectively_mcp_hidden`). Plaintext local
/// flag — no vault involved, and (unlike `locked`) never synced.
#[tauri::command]
pub fn folder_set_mcp_hidden(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    id: String,
    hidden: bool,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        store
            .set_folder_mcp_hidden(&id, hidden)
            .map_err(|e| e.to_string())?;
    }
    notify(&app, &webview);
    Ok(())
}

// ---------------------------------------------------------------------------
// Contexts
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn contexts_list(
    reg: State<'_, Mutex<crate::profiles::Registry>>,
) -> Result<Vec<ContextInfo>, String> {
    let r = reg.lock().map_err(|e| e.to_string())?;
    Ok(ops::to_infos_with(
        &r,
        |c| ops::context_vault_exists(std::path::Path::new(&c.path)),
        |c| crate::vault::biometric::is_available() && crate::vault::biometric::is_enrolled(&c.id),
    ))
}

#[tauri::command]
pub fn context_add(
    app: AppHandle,
    reg: State<'_, Mutex<crate::profiles::Registry>>,
    store: State<'_, Mutex<Store>>,
    label: String,
) -> Result<Vec<ContextInfo>, String> {
    let (path, infos) = {
        let mut r = reg.lock().map_err(|e| e.to_string())?;
        ops::context_add(
            &mut r,
            &crate::config::contexts_dir(&app),
            &crate::config::profiles_path(&app),
            label,
        )?
    };
    swap_store_to(&app, &store, &path, false)?;
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
    let (path, is_server) = {
        let mut r = reg.lock().map_err(|e| e.to_string())?;
        ops::context_switch(&mut r, &crate::config::profiles_path(&app), &id)?
    };
    swap_store_to(&app, &store, std::path::Path::new(&path), is_server)?;
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
    ops::context_rename(&mut r, &crate::config::profiles_path(&app), &id, label)
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
        ops::context_remove(
            &mut r,
            &crate::config::profiles_path(&app),
            &id,
            delete_file,
        )?
    };
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
    let authorize = ops::build_authorize_url(&config, &p.challenge, &p.state)?;

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
    let (code, state) = ops::parse_auth_callback(&url)?;

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
    let path = ops::prepare_context_db(&crate::config::contexts_dir(&app), &id)?;
    crate::auth::store_tokens(&id, &tokens)?;

    let label = ops::server_label(&pa.server_url);
    let infos = {
        let mut r = reg.lock().map_err(|e| e.to_string())?;
        ops::register_server_context(
            &mut r,
            &crate::config::profiles_path(&app),
            &id,
            label,
            &path,
            pa.server_url,
        )?
    };
    swap_store_to(&app, &store, &path, true)?;
    broadcast_context_changed(&app);
    Ok(infos)
}

// Lock convention (three managed mutexes: Registry, Store, VaultState).
//
// When two are held at once, the order is always:
//
//     Registry -> Store -> VaultState
//
// and never the reverse. Registry -> Store is nested by `sync_status` and by
// `widgetshare::publish`; Store -> VaultState is nested by `notes_load_one`,
// `notes_save`, `notes_set_folder`, `notes_reorder`, `note_set_protected`,
// `folder_set_locked`, `vault_unlock_biometric`/`vault_unlock`/
// `vault_unlock_recovery` (each locks Store, then re-locks VaultState inside
// that block to read the live ring for `ops::backfill_protected_titles`),
// `mcp::StoreAccess::decrypt_protected` and `mcp::StoreAccess::write_protected`.
// Nothing takes Store before Registry, or VaultState before Store, while
// holding the other.
//
// `swap_store_to` below is the one place VaultState is touched before the
// Store — but it takes and DROPS that guard in its own block first, so the two
// are never held simultaneously and the convention still holds. Everything
// else that needs both simply scopes the first guard so it is released before
// the second is taken.
fn swap_store_to(
    app: &AppHandle,
    store: &State<'_, Mutex<Store>>,
    path: &std::path::Path,
    sync_enabled: bool,
) -> Result<(), String> {
    // The active context DB is changing. The unlocked DEK belongs to the
    // previous context's vault and must never touch another context's
    // ciphertext — decrypting would fail, and sealing a note under the wrong
    // DEK would make it unrecoverable. Lock the vault before swapping (and
    // before taking the store lock, so the two mutexes are never nested).
    {
        let vs = app.state::<Mutex<crate::vault::state::VaultState>>();
        let mut guard = vs.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.lock();
    }
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

// ---------------------------------------------------------------------------
// Storage location, revisions, MCP, windows
// ---------------------------------------------------------------------------

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
        ops::move_db_files(&current, &target)?;
        "moved"
    };

    crate::config::write_db_path(&app, &target).map_err(|e| e.to_string())?;

    // Reopen at the target so the running app stays consistent until relaunch.
    {
        let mut s = store.lock().map_err(|e| e.to_string())?;
        ops::reopen_store_at(&mut s, &target)?;
    }

    // Keep the active context's registry entry pointing at the new path.
    if let Some(reg) = app.try_state::<Mutex<crate::profiles::Registry>>() {
        if let Ok(mut r) = reg.lock() {
            ops::point_active_context_at(&mut r, &target);
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
    ops::save_image(&crate::images::images_dir(&app), &note_id, &name, &bytes)
}

#[tauri::command]
pub fn check_paths(app: AppHandle) -> PathChecks {
    ops::check_paths(
        &crate::config::read_db_path(&app),
        &crate::images::images_dir(&app),
    )
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn export_notes_base64(
    store: State<'_, Mutex<Store>>,
    app: AppHandle,
    path: String,
    ids: Vec<String>,
) -> Result<(), String> {
    let root = crate::images::images_dir(&app);
    let notes = {
        let s = store.lock().map_err(|e| e.to_string())?;
        s.load_notes().map_err(|e| e.to_string())?
    };
    ops::export_notes_inlined(notes, &root, std::path::Path::new(&path), &ids)
}

#[tauri::command]
pub fn note_inlined_html(
    store: State<'_, Mutex<Store>>,
    app: AppHandle,
    note_id: String,
) -> Result<String, String> {
    let root = crate::images::images_dir(&app);
    let notes = {
        let s = store.lock().map_err(|e| e.to_string())?;
        s.load_all_notes().map_err(|e| e.to_string())?
    };
    ops::note_inlined_html(notes, &root, &note_id)
}

#[tauri::command]
pub fn save_export(path: String, bytes: Vec<u8>) -> Result<(), String> {
    ops::save_export(std::path::Path::new(&path), &bytes)
}

#[tauri::command]
pub fn export_md_bundle(
    app: AppHandle,
    dir: String,
    md: String,
    name: String,
) -> Result<(), String> {
    ops::export_md_bundle(
        &crate::images::images_dir(&app),
        std::path::Path::new(&dir),
        &md,
        &name,
    )
}

#[tauri::command]
pub fn export_notes_bundle(
    store: State<'_, Mutex<Store>>,
    app: AppHandle,
    dir: String,
    ids: Vec<String>,
) -> Result<(), String> {
    let root = crate::images::images_dir(&app);
    let notes = {
        let s = store.lock().map_err(|e| e.to_string())?;
        s.load_notes().map_err(|e| e.to_string())?
    };
    ops::export_notes_bundle(notes, &root, std::path::Path::new(&dir), &ids)
}

// ---------------------------------------------------------------------------
// Sync
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn server_workspaces(
    reg: State<'_, Mutex<crate::profiles::Registry>>,
) -> Result<Vec<crate::sync::WorkspaceInfo>, String> {
    let ctx = {
        let r = reg.lock().map_err(|e| e.to_string())?;
        ops::active_server(&r).ok_or("no active server context")?
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
        ops::context_bind_workspace(
            &mut r,
            &crate::config::profiles_path(&app),
            &id,
            workspace_id,
            label,
        )?
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
    if let Some(status) = ops::sync_status_local(&r) {
        return Ok(status);
    }
    let s = store.lock().map_err(|e| e.to_string())?;
    ops::sync_status_synced(&s)
}

/// One push-then-pull cycle for the active server context. Locks are released
/// before every network `.await`. No-op for local/unbound/unauthenticated.
pub async fn run_sync_cycle(app: &AppHandle) -> Result<(), String> {
    let reg_state = app.state::<Mutex<crate::profiles::Registry>>();
    let store_state = app.state::<Mutex<Store>>();

    let ctx = {
        let r = reg_state.lock().map_err(|e| e.to_string())?;
        ops::active_server(&r)
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
    let ops::SyncPush {
        folders,
        notes,
        note_ids,
        folder_ids,
        since,
    } = {
        let s = store_state.lock().map_err(|e| e.to_string())?;
        ops::collect_sync_push(&s)?
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
        Ok(pull) => {
            {
                let s = store_state.lock().map_err(|e| e.to_string())?;
                ops::commit_sync_result(&s, &note_ids, &folder_ids, &pull, now_ms())?;
            }
            broadcast_context_changed(app); // refresh the UI from the updated cache
                                            // A vault set up while this context was local (or offline)
                                            // joins the workspace here. Non-fatal — a network failure is
                                            // retried on the next cycle, exactly like the image phase.
            let _ = run_vault_migration(app, &ctx, &tokens.access_token).await;
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
    let r = reg.lock().map_err(|e| e.to_string())?;
    Ok(crate::aggregate::aggregate_meta(&ops::registry_contexts(
        &r,
    )))
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
    let unlocked = vault.lock().map_err(|e| e.to_string())?.is_unlocked();
    let contexts = {
        let r = reg.lock().map_err(|e| e.to_string())?;
        ops::registry_contexts(&r)
    };
    Ok(ops::search_all_contexts(&contexts, &query, unlocked))
}

/// After a successful sync round-trip: put a local-only vault record onto the
/// workspace, or record that the workspace already holds a vault this record
/// did not create (meta `vault_conflict` — the UI surfaces it, and a
/// successful unlock from the workspace entries clears it again, since the
/// same DEK proves the two are one vault).
///
/// Non-fatal by design: the note sync is already committed, so a network
/// failure here is swallowed by the caller and retried next cycle. No Store
/// lock is held across the `.await`.
pub async fn run_vault_migration(
    app: &AppHandle,
    ctx: &crate::profiles::ContextEntry,
    token: &str,
) -> Result<(), String> {
    let store_state = app.state::<Mutex<Store>>();
    let plan = {
        let s = store_state.lock().map_err(|e| e.to_string())?;
        ops::vault_migration_plan(&s)?
    };
    let outcome = match plan {
        ops::VaultMigration::None => return Ok(()),
        // The workspace already has a vault; no point asking it again.
        ops::VaultMigration::Conflict => "vault_conflict",
        ops::VaultMigration::Upload(payload) => {
            match crate::sync::vault_create(&ctx.server_url, token, &ctx.workspace_id, &payload)
                .await
                .map_err(|e| e.to_string())?
            {
                crate::sync::VaultCreateOutcome::Created => "vault_migrated",
                crate::sync::VaultCreateOutcome::AlreadyExists => "vault_conflict",
            }
        }
    };
    let s = store_state.lock().map_err(|e| e.to_string())?;
    ops::record_vault_migration(&s, outcome)
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
    let local_present: HashSet<String> = ops::locally_present_images(&referenced, &images_root);

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

// ---------------------------------------------------------------------------
// Vault
//
// The unlocked DEK lives only in the managed `VaultState` — it is never
// logged, persisted, or returned to the frontend. Only `vault_setup` returns
// secret material, and only the recovery key (shown once, by design).
// ---------------------------------------------------------------------------

type VaultStateHandle<'r> = State<'r, Mutex<crate::vault::state::VaultState>>;

/// The active context's id — the scope of its biometric keychain item.
fn active_context_id(reg: &State<'_, Mutex<crate::profiles::Registry>>) -> Result<String, String> {
    let reg = reg.lock().map_err(|e| e.to_string())?;
    reg.active()
        .map(|c| c.id.clone())
        .ok_or_else(|| "no active context".to_string())
}

/// Installs every unlocked `(generation, DEK)` into the ring.
fn install_generations(
    vault: &VaultStateHandle<'_>,
    opened: &[(u32, crate::vault::aead::Dek)],
) -> Result<(), String> {
    let mut v = vault.lock().map_err(|e| e.to_string())?;
    for (generation, dek) in opened {
        v.unlock(*generation, dek.clone());
    }
    Ok(())
}

/// Where this context's vault keys live on the server, if they do: a bound
/// server context plus a usable access token. `None` — meaning "purely local
/// vault" — for a local context, an unbound or unauthenticated one, and for a
/// server that predates workspace vault keys (meta `vault_server_legacy`,
/// set by `ops::apply_vault_keys` when a pull carried no `vaultKeys`).
///
/// Takes the Registry and Store locks one after the other, never nested —
/// Registry -> Store, per the lock-order convention near `swap_store_to`.
fn vault_server_target(
    app: &AppHandle,
) -> Result<Option<(crate::profiles::ContextEntry, String)>, String> {
    let reg_state = app.state::<Mutex<crate::profiles::Registry>>();
    let store_state = app.state::<Mutex<Store>>();
    let ctx = {
        let r = reg_state.lock().map_err(|e| e.to_string())?;
        ops::active_server(&r)
    };
    let Some(ctx) = ctx.filter(|c| !c.workspace_id.is_empty()) else {
        return Ok(None);
    };
    let legacy = {
        let s = store_state.lock().map_err(|e| e.to_string())?;
        crate::migrate::get_meta_i64_opt(&s.conn, "vault_server_legacy")
            .map_err(|e| e.to_string())?
            .is_some()
    };
    if legacy {
        return Ok(None);
    }
    Ok(crate::auth::load_tokens(&ctx.id)?.map(|t| (ctx, t.access_token)))
}

#[tauri::command]
pub fn vault_status(
    store: State<'_, Mutex<Store>>,
    vault: VaultStateHandle<'_>,
    reg: State<'_, Mutex<crate::profiles::Registry>>,
) -> Result<VaultStatus, String> {
    let context_id = active_context_id(&reg)?;
    let exists = {
        let store = store.lock().map_err(|e| e.to_string())?;
        // A device that has only pulled its wrapped keys has no local record
        // yet, but its vault definitely exists — it must be offered "unlock",
        // never "set up" (which would mint a second, incompatible DEK).
        ops::vault_exists(&store)?
    };
    let unlocked = vault.lock().map_err(|e| e.to_string())?.is_unlocked();
    Ok(VaultStatus {
        exists,
        unlocked,
        // Biometric unlock is offered only when the device can evaluate Touch
        // ID (`is_available`) AND the user has enrolled a wrapped DEK
        // (`is_enrolled`). `is_enrolled` reads the keychain without prompting.
        biometric: crate::vault::biometric::is_available()
            && crate::vault::biometric::is_enrolled(&context_id),
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
pub fn vault_biometric_enable(
    vault: VaultStateHandle<'_>,
    reg: State<'_, Mutex<crate::profiles::Registry>>,
) -> Result<(), String> {
    let context_id = active_context_id(&reg)?;
    let vault = vault.lock().map_err(|e| e.to_string())?;
    let dek = vault.dek().ok_or_else(|| "vault locked".to_string())?;
    crate::vault::biometric::store_dek(&context_id, dek).map_err(String::from)
}

/// Disables biometric unlock by deleting the keychain-stored DEK. Idempotent.
#[tauri::command]
pub fn vault_biometric_disable(
    reg: State<'_, Mutex<crate::profiles::Registry>>,
) -> Result<(), String> {
    let context_id = active_context_id(&reg)?;
    crate::vault::biometric::clear(&context_id).map_err(String::from)
}

/// Unlocks the vault via biometrics: prompt Touch ID, then release the
/// keychain-wrapped DEK into `VaultState`. Async so the blocking Touch ID
/// prompt runs off the main thread (`spawn_blocking`) — otherwise the main run
/// loop would be blocked and could not present the system dialog.
#[tauri::command]
pub async fn vault_unlock_biometric(
    store: State<'_, Mutex<Store>>,
    vault: VaultStateHandle<'_>,
    reg: State<'_, Mutex<crate::profiles::Registry>>,
) -> Result<(), String> {
    let context_id = active_context_id(&reg)?;
    tauri::async_runtime::spawn_blocking(|| {
        crate::vault::biometric::authenticate("Unlock your protected notes")
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(String::from)?;
    let dek = crate::vault::biometric::load_dek(&context_id)
        .map_err(String::from)?
        .ok_or_else(|| "vault: biometric unlock is not set up".to_string())?;
    // The keychain item carries no proof of ownership: prove the DEK opens
    // THIS context's vault before it can seal anything (see verify_dek).
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        ops::verify_dek_for_store(&store, &dek)?;
    }
    vault.lock().map_err(|e| e.to_string())?.unlock(1, dek);
    if let Ok(store) = store.lock() {
        // Store -> VaultState, per the lock-order convention near `swap_store_to`.
        if let Ok(v) = vault.lock() {
            ops::backfill_protected_titles(&store, &v);
        }
    }
    Ok(())
}

/// Creates a new vault: wraps a fresh DEK under `passphrase`, persists the
/// record, and unlocks it immediately. Returns the one-time recovery key
/// split into its dash-separated groups — the only place it is ever exposed.
///
/// In a bound server context the workspace is the home of the wrapped keys,
/// so the record this device just stored is uploaded as generation 1. The
/// local setup runs first and the upload is built from the stored record, so
/// device and workspace can never end up holding different vaults. If the
/// workspace already has one (another device won the race), the local record
/// is rolled back and the user is sent to the unlock screen — a half-created
/// local vault the server never saw must not survive.
///
/// Async because of that upload; the frontend calls it exactly as before.
#[tauri::command]
pub async fn vault_setup(app: AppHandle, passphrase: String) -> Result<Vec<String>, String> {
    let target = vault_server_target(&app)?;
    let store_state = app.state::<Mutex<Store>>();

    let (groups, dek, payload) = {
        let store = store_state.lock().map_err(|e| e.to_string())?;
        let (groups, dek) = ops::vault_setup(&store, &passphrase)?;
        let payload = match target.is_some() {
            true => Some(ops::migration_payload(&ops::load_vault_record(&store)?)?),
            false => None,
        };
        (groups, dek, payload)
    };

    if let (Some((ctx, token)), Some(payload)) = (target, payload) {
        let outcome =
            crate::sync::vault_create(&ctx.server_url, &token, &ctx.workspace_id, &payload).await;
        let rejected = match outcome {
            Ok(crate::sync::VaultCreateOutcome::Created) => None,
            Ok(crate::sync::VaultCreateOutcome::AlreadyExists) => Some(
                "vault: already set up on the server — unlock with your passphrase".to_string(),
            ),
            Err(e) => Some(e.to_string()),
        };
        let store = store_state.lock().map_err(|e| e.to_string())?;
        match rejected {
            None => ops::record_vault_migration(&store, "vault_migrated")?,
            Some(e) => {
                let _ = store.clear_vault_record();
                return Err(e);
            }
        }
    }

    app.state::<Mutex<crate::vault::state::VaultState>>()
        .lock()
        .map_err(|e| e.to_string())?
        .unlock(1, dek);
    Ok(groups)
}

/// Shared body of [`vault_unlock`] and [`vault_unlock_recovery`].
///
/// Where the workspace has handed this caller a wrapped key per generation
/// (cached by `ops::apply_vault_keys` on every pull), the cache is the way
/// in: `ops::plan_entry_unlock` decides which generations may be installed
/// and whether the workspace vault is provably this device's own vault
/// before anything is written back. Otherwise the local record is the only
/// way in, exactly as before.
///
/// The store lock is held only for the reads and for the write-back — never
/// across the KDF derivations, which would stall every other store consumer.
/// Lock order stays Store -> VaultState.
fn unlock_vault_with(
    store: &State<'_, Mutex<Store>>,
    vault: &VaultStateHandle<'_>,
    secret: ops::VaultSecret<'_>,
) -> Result<(), String> {
    let inputs = {
        let store = store.lock().map_err(|e| e.to_string())?;
        ops::load_vault_unlock_inputs(&store)?
    };

    let Some(entries) = inputs.entries.filter(|e| secret.entries_usable(e)) else {
        // Purely local vault: today's path.
        let record = inputs
            .record
            .ok_or_else(|| "vault: not set up".to_string())?;
        let dek = secret.open(&record)?;
        let backfill_dek = dek.clone();
        vault.lock().map_err(|e| e.to_string())?.unlock(1, dek);
        if let Ok(store) = store.lock() {
            ops::ensure_dek_check(&store, &record, &backfill_dek);
            // Store -> VaultState, per the lock-order convention near `swap_store_to`.
            if let Ok(v) = vault.lock() {
                ops::backfill_protected_titles(&store, &v);
            }
        }
        return Ok(());
    };

    let plan = ops::plan_entry_unlock(inputs.record.as_ref(), &entries, &secret)?;
    install_generations(vault, &plan.install)?;
    if let Ok(store) = store.lock() {
        ops::apply_entry_unlock(&store, inputs.record.as_ref(), &entries, &plan)?;
        // Store -> VaultState, per the lock-order convention near `swap_store_to`.
        if let Ok(v) = vault.lock() {
            ops::backfill_protected_titles(&store, &v);
        }
    }
    Ok(())
}

/// Unlocks the vault with `passphrase`. Stays synchronous: the entry cache
/// is local, so no network is involved.
#[tauri::command]
pub fn vault_unlock(
    store: State<'_, Mutex<Store>>,
    vault: VaultStateHandle<'_>,
    passphrase: String,
) -> Result<(), String> {
    unlock_vault_with(&store, &vault, ops::VaultSecret::Passphrase(&passphrase))
}

/// [`vault_unlock`] via the one-time recovery key — the workspace's recovery
/// wraps when it has them, the local record otherwise.
#[tauri::command]
pub fn vault_unlock_recovery(
    store: State<'_, Mutex<Store>>,
    vault: VaultStateHandle<'_>,
    recovery: String,
) -> Result<(), String> {
    unlock_vault_with(&store, &vault, ops::VaultSecret::Recovery(&recovery))
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
///
/// In a bound server context the workspace holds the wraps, so EVERY
/// generation is rewrapped and PUT back. Strict order — compute, then
/// network, then persist: nothing is written locally until every upload has
/// landed, so a failed PUT leaves this device on `current`, exactly in step
/// with the workspace.
#[tauri::command]
pub async fn vault_change_passphrase(
    app: AppHandle,
    current: String,
    next: String,
) -> Result<(), String> {
    let target = vault_server_target(&app)?;
    let store_state = app.state::<Mutex<Store>>();
    let vault_state = app.state::<Mutex<crate::vault::state::VaultState>>();

    // Rewrap the single local record in place — the path for a local
    // context, a legacy server, and a workspace that holds no key for this
    // caller yet (the migration hook has not run, or it hit a conflict).
    let local_only = || -> Result<(), String> {
        let dek = {
            let store = store_state.lock().map_err(|e| e.to_string())?;
            ops::vault_change_passphrase(&store, &current, &next)?
        };
        vault_state
            .lock()
            .map_err(|e| e.to_string())?
            .unlock(1, dek);
        Ok(())
    };

    let Some((ctx, token)) = target else {
        return local_only();
    };

    // (a) read both halves under the lock, then drop it.
    let inputs = {
        let store = store_state.lock().map_err(|e| e.to_string())?;
        ops::load_vault_unlock_inputs(&store)?
    };
    let Some(entries) = inputs.entries.filter(|e| !e.mine.is_empty()) else {
        return local_only();
    };

    // (b) rewrap in memory — no store writes, no network.
    let rewrap = ops::rewrap_for_server(inputs.record.as_ref(), &entries, &current, &next)?;

    // (c) upload first; any failure aborts before a single local write.
    for entry in &rewrap.uploads {
        crate::sync::vault_put_my_key(&ctx.server_url, &token, &ctx.workspace_id, entry)
            .await
            .map_err(|e| e.to_string())?;
    }

    // (d) every generation landed: persist, then re-arm the ring.
    {
        let store = store_state.lock().map_err(|e| e.to_string())?;
        if let Some(record) = &rewrap.record {
            store
                .set_vault_record(&record.to_json())
                .map_err(|e| e.to_string())?;
        }
        store
            .set_vault_entries(&rewrap.entries.to_json())
            .map_err(|e| e.to_string())?;
    }
    install_generations(&vault_state, &rewrap.deks)
}

/// Changes a context's vault passphrase from the Kontexte page, without
/// switching into it first. For the ACTIVE context this is exactly
/// `vault_change_passphrase` above (managed store + re-arming
/// `VaultState`); for any other context it delegates to
/// `ops::change_context_vault_passphrase`, which opens that context's own
/// DB, rewraps its DEK there, and never touches `VaultState` — that state
/// always tracks the active context only.
#[tauri::command]
pub async fn context_vault_change_passphrase(
    app: AppHandle,
    reg: State<'_, Mutex<crate::profiles::Registry>>,
    id: String,
    current: String,
    next: String,
) -> Result<(), String> {
    let is_active = {
        let r = reg.lock().map_err(|e| e.to_string())?;
        r.active_id == id
    };
    if is_active {
        // Async now, because the active context may have to rewrap its keys
        // on the server too — the registry lock above is already released.
        return vault_change_passphrase(app, current, next).await;
    }
    let r = reg.lock().map_err(|e| e.to_string())?;
    ops::change_context_vault_passphrase(&r, &id, &current, &next)
}

/// Encrypts or decrypts one note's stored content in place — see
/// `ops::set_note_protected` for the policy (including the refusal to
/// unprotect a note that sits inside a locked folder).
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
        ops::set_note_protected(&store, &vault, &id, protected)?;
    }
    notify(&app, &webview);
    Ok(())
}

/// Locks or unlocks a folder, encrypting/decrypting the notes in its subtree
/// to match — see `ops::set_folder_locked`.
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
        ops::set_folder_locked(&store, &vault, &id, locked)?;
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

    #[test]
    fn query_and_fragment_are_stripped_before_routing() {
        assert_eq!(
            parse_widget_url("notefix://new?from=widget"),
            WidgetAction::NewNote
        );
        assert_eq!(
            parse_widget_url("notefix://note/n1?x=1"),
            WidgetAction::OpenNote("n1".into())
        );
        assert_eq!(
            parse_widget_url("notefix://note/n1#frag"),
            WidgetAction::OpenNote("n1".into())
        );
    }

    #[test]
    fn unknown_host_and_empty_path_fall_back_to_auth() {
        assert_eq!(parse_widget_url("notefix://"), WidgetAction::Auth);
        assert_eq!(parse_widget_url("notefix:///"), WidgetAction::Auth);
        assert_eq!(parse_widget_url("notefix://unknown/x"), WidgetAction::Auth);
        assert_eq!(
            parse_widget_url("https://example.test/new"),
            WidgetAction::Auth
        );
    }

    #[test]
    fn only_the_first_path_segment_after_note_is_the_id() {
        assert_eq!(
            parse_widget_url("notefix://note/n1/extra"),
            WidgetAction::OpenNote("n1".into())
        );
    }
}
