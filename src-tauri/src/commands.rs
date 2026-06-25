use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::storage::{Note, Store};

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
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

#[tauri::command]
pub fn notes_load(store: State<'_, Mutex<Store>>) -> Result<Vec<Note>, String> {
    let store = store.lock().map_err(|e| e.to_string())?;
    store.load_notes().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn notes_save(
    app: AppHandle,
    webview: WebviewWindow,
    store: State<'_, Mutex<Store>>,
    note: Note,
) -> Result<(), String> {
    {
        let store = store.lock().map_err(|e| e.to_string())?;
        store.save_note(&note).map_err(|e| e.to_string())?;
        let limit = crate::settings::get_int(&store.conn, "revisionLimit", 50);
        crate::revisions::add_revision(&store.conn, &note.id, &note.content, limit).map_err(|e| e.to_string())?;
    }
    broadcast_changed(&app, webview.label());
    crate::tray::rebuild_menu(&app);
    Ok(())
}

#[tauri::command]
pub fn notes_delete(app: AppHandle, webview: WebviewWindow, store: State<'_, Mutex<Store>>, id: String) -> Result<(), String> {
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
pub fn notes_restore(app: AppHandle, webview: WebviewWindow, store: State<'_, Mutex<Store>>, id: String) -> Result<(), String> {
    { let store = store.lock().map_err(|e| e.to_string())?; store.restore_note(&id).map_err(|e| e.to_string())?; }
    broadcast_changed(&app, webview.label());
    crate::tray::rebuild_menu(&app);
    Ok(())
}

#[tauri::command]
pub fn notes_purge(app: AppHandle, webview: WebviewWindow, store: State<'_, Mutex<Store>>, id: String) -> Result<(), String> {
    { let store = store.lock().map_err(|e| e.to_string())?; store.delete_note(&id).map_err(|e| e.to_string())?; crate::images::run_gc(&app, &store); }
    broadcast_changed(&app, webview.label());
    crate::tray::rebuild_menu(&app);
    Ok(())
}

#[tauri::command]
pub fn trash_load(store: State<'_, Mutex<Store>>) -> Result<Vec<Note>, String> {
    let store = store.lock().map_err(|e| e.to_string())?;
    store.load_trashed().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn trash_empty(app: AppHandle, webview: WebviewWindow, store: State<'_, Mutex<Store>>) -> Result<(), String> {
    { let store = store.lock().map_err(|e| e.to_string())?; store.purge_trashed(None).map_err(|e| e.to_string())?; crate::images::run_gc(&app, &store); }
    broadcast_changed(&app, webview.label());
    crate::tray::rebuild_menu(&app);
    Ok(())
}

/// Open a frameless floating window for a note. Tauri requires unique labels,
/// so re-opening the same note focuses the existing window instead of duplicating.
#[tauri::command]
pub async fn open_note_window(app: AppHandle, note_id: String) -> Result<(), String> {
    let label = format!("note-{note_id}");
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
        store.set_archived(&id, archived).map_err(|e| e.to_string())?;
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
pub fn folder_create(app: AppHandle, webview: WebviewWindow, store: State<'_, Mutex<Store>>, id: String, name: String, parent_id: Option<String>) -> Result<(), String> {
    { let store = store.lock().map_err(|e| e.to_string())?; crate::folders::create_folder(&store.conn, &id, &name, parent_id.as_deref()).map_err(|e| e.to_string())?;
      if store.sync_enabled { crate::folders::touch_folder(&store.conn, &id).map_err(|e| e.to_string())?; } }
    notify(&app, &webview);
    Ok(())
}

#[tauri::command]
pub fn folder_rename(app: AppHandle, webview: WebviewWindow, store: State<'_, Mutex<Store>>, id: String, name: String) -> Result<(), String> {
    { let store = store.lock().map_err(|e| e.to_string())?; crate::folders::rename_folder(&store.conn, &id, &name).map_err(|e| e.to_string())?;
      if store.sync_enabled { crate::folders::touch_folder(&store.conn, &id).map_err(|e| e.to_string())?; } }
    notify(&app, &webview);
    Ok(())
}

#[tauri::command]
pub fn folder_move(app: AppHandle, webview: WebviewWindow, store: State<'_, Mutex<Store>>, id: String, parent_id: Option<String>) -> Result<(), String> {
    { let store = store.lock().map_err(|e| e.to_string())?; crate::folders::move_folder(&store.conn, &id, parent_id.as_deref()).map_err(|e| e.to_string())?;
      if store.sync_enabled { crate::folders::touch_folder(&store.conn, &id).map_err(|e| e.to_string())?; } }
    notify(&app, &webview);
    Ok(())
}

#[tauri::command]
pub fn folder_delete(app: AppHandle, webview: WebviewWindow, store: State<'_, Mutex<Store>>, id: String, mode: String) -> Result<(), String> {
    { let store = store.lock().map_err(|e| e.to_string())?;
      if store.sync_enabled {
          crate::folders::sync_delete_folder(&store.conn, &id, crate::folders::DeleteMode::from_str(&mode)).map_err(|e| e.to_string())?;
      } else {
          crate::folders::delete_folder(&store.conn, &id, crate::folders::DeleteMode::from_str(&mode)).map_err(|e| e.to_string())?;
      }
      crate::images::run_gc(&app, &store); }
    notify(&app, &webview);
    Ok(())
}

#[tauri::command]
pub fn notes_set_folder(app: AppHandle, webview: WebviewWindow, store: State<'_, Mutex<Store>>, id: String, folder_id: Option<String>) -> Result<(), String> {
    { let store = store.lock().map_err(|e| e.to_string())?; store.set_folder(&id, folder_id.as_deref()).map_err(|e| e.to_string())?; }
    notify(&app, &webview);
    Ok(())
}

#[tauri::command]
pub fn notes_reorder(app: AppHandle, webview: WebviewWindow, store: State<'_, Mutex<Store>>, folder_id: Option<String>, ids: Vec<String>) -> Result<(), String> {
    { let store = store.lock().map_err(|e| e.to_string())?; store.reorder_notes(folder_id.as_deref(), &ids).map_err(|e| e.to_string())?; }
    notify(&app, &webview);
    Ok(())
}

#[tauri::command]
pub fn folders_reorder(app: AppHandle, webview: WebviewWindow, store: State<'_, Mutex<Store>>, parent_id: Option<String>, ids: Vec<String>) -> Result<(), String> {
    { let store = store.lock().map_err(|e| e.to_string())?; crate::folders::reorder_folders(&store.conn, parent_id.as_deref(), &ids).map_err(|e| e.to_string())?; }
    notify(&app, &webview);
    Ok(())
}

#[tauri::command]
pub fn folder_set_icon(app: AppHandle, webview: WebviewWindow, store: State<'_, Mutex<Store>>, id: String, icon: String) -> Result<(), String> {
    { let store = store.lock().map_err(|e| e.to_string())?; crate::folders::set_folder_icon(&store.conn, &id, &icon).map_err(|e| e.to_string())?;
      if store.sync_enabled { crate::folders::touch_folder(&store.conn, &id).map_err(|e| e.to_string())?; } }
    notify(&app, &webview);
    Ok(())
}

#[tauri::command]
pub fn folder_set_color(app: AppHandle, webview: WebviewWindow, store: State<'_, Mutex<Store>>, id: String, color: String) -> Result<(), String> {
    { let store = store.lock().map_err(|e| e.to_string())?; crate::folders::set_folder_color(&store.conn, &id, &color).map_err(|e| e.to_string())?;
      if store.sync_enabled { crate::folders::touch_folder(&store.conn, &id).map_err(|e| e.to_string())?; } }
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
    reg.contexts.iter().map(|c| ContextInfo {
        id: c.id.clone(), label: c.label.clone(), kind: c.kind.clone(),
        path: c.path.clone(), server_url: c.server_url.clone(),
        workspace_id: c.workspace_id.clone(), active: c.id == reg.active_id,
    }).collect()
}

#[tauri::command]
pub fn contexts_list(reg: State<'_, Mutex<crate::profiles::Registry>>) -> Result<Vec<ContextInfo>, String> {
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
    { let s = Store::open(&path).map_err(|e| e.to_string())?; crate::migrate::run_migrations(&s.conn).map_err(|e| e.to_string())?; }
    let infos = {
        let mut r = reg.lock().map_err(|e| e.to_string())?;
        r.add(id.clone(), label, path.to_string_lossy().into_owned());
        r.set_active(&id)?;
        crate::profiles::save(&crate::config::profiles_path(&app), &r).map_err(|e| e.to_string())?;
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
        crate::profiles::save(&crate::config::profiles_path(&app), &r).map_err(|e| e.to_string())?;
        (p, kind)
    };
    swap_store_to(&store, std::path::Path::new(&path), kind == "server")?;
    broadcast_context_changed(&app);
    Ok(())
}

#[tauri::command]
pub fn context_rename(app: AppHandle, reg: State<'_, Mutex<crate::profiles::Registry>>, id: String, label: String) -> Result<Vec<ContextInfo>, String> {
    let mut r = reg.lock().map_err(|e| e.to_string())?;
    r.rename(&id, label)?;
    crate::profiles::save(&crate::config::profiles_path(&app), &r).map_err(|e| e.to_string())?;
    Ok(to_infos(&r))
}

#[tauri::command]
pub fn context_remove(app: AppHandle, reg: State<'_, Mutex<crate::profiles::Registry>>, id: String, delete_file: bool) -> Result<Vec<ContextInfo>, String> {
    let (removed, infos) = {
        let mut r = reg.lock().map_err(|e| e.to_string())?;
        let removed = r.remove(&id)?;
        crate::profiles::save(&crate::config::profiles_path(&app), &r).map_err(|e| e.to_string())?;
        (removed, to_infos(&r))
    };
    if delete_file {
        for ext in ["", "-wal", "-shm"] { let p = with_ext(std::path::Path::new(&removed.path), ext); let _ = std::fs::remove_file(p); }
    }
    // Server contexts keep their tokens in the keychain; drop them on removal.
    if removed.kind == "server" { let _ = crate::auth::clear_tokens(&removed.id); }
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

    pending
        .lock()
        .map_err(|e| e.to_string())?
        .insert(p.state, PendingAuth { verifier: p.verifier, server_url, config });
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

    let tokens =
        crate::auth::exchange_code(&pa.config.token_url, &pa.config.client_id, &code, &pa.verifier)
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
        r.add_server(id.clone(), label, path.to_string_lossy().into_owned(), pa.server_url);
        r.set_active(&id)?;
        crate::profiles::save(&crate::config::profiles_path(&app), &r).map_err(|e| e.to_string())?;
        to_infos(&r)
    };
    swap_store_to(&store, &path, true)?;
    broadcast_context_changed(&app);
    Ok(infos)
}

// Lock convention: never hold the Store and Registry locks simultaneously; if ever needed, lock Store before Registry.
fn swap_store_to(store: &State<'_, Mutex<Store>>, path: &std::path::Path, sync_enabled: bool) -> Result<(), String> {
    let mut s = store.lock().map_err(|e| e.to_string())?;
    let opened = Store::open(path).map_err(|e| e.to_string())?;
    s.conn = opened.conn;
    s.sync_enabled = sync_enabled;
    crate::migrate::run_migrations(&s.conn).map_err(|e| e.to_string())?;
    Ok(())
}

fn broadcast_context_changed(app: &AppHandle) {
    let labels: Vec<String> = app.webview_windows().keys().cloned().collect();
    for label in labels { let _ = app.emit_to(label.as_str(), "context-changed", ()); }
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
    crate::config::read_db_path(&app).to_string_lossy().into_owned()
}

#[tauri::command]
pub fn set_db_location(app: AppHandle, store: State<'_, Mutex<Store>>, folder: String) -> Result<DbLocationResult, String> {
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
            if let Some(c) = r.contexts.iter_mut().find(|c| c.id == active) { c.path = target.to_string_lossy().into_owned(); }
            let _ = crate::profiles::save(&crate::config::profiles_path(&app), &r);
        }
    }

    Ok(DbLocationResult { mode: mode.to_string(), path: target.to_string_lossy().into_owned() })
}

#[tauri::command]
pub fn note_revisions(store: State<'_, Mutex<Store>>, note_id: String) -> Result<Vec<crate::revisions::Revision>, String> {
    let store = store.lock().map_err(|e| e.to_string())?;
    crate::revisions::list_revisions(&store.conn, &note_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn note_revision_content(store: State<'_, Mutex<Store>>, id: i64) -> Result<Option<String>, String> {
    let store = store.lock().map_err(|e| e.to_string())?;
    crate::revisions::revision_content(&store.conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn folder_set_sort(app: AppHandle, webview: WebviewWindow, store: State<'_, Mutex<Store>>, id: String, sort: String) -> Result<(), String> {
    { let store = store.lock().map_err(|e| e.to_string())?; crate::folders::set_folder_sort(&store.conn, &id, &sort).map_err(|e| e.to_string())?;
      if store.sync_enabled { crate::folders::touch_folder(&store.conn, &id).map_err(|e| e.to_string())?; } }
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
pub fn save_image(app: AppHandle, note_id: String, name: String, bytes: Vec<u8>) -> Result<String, String> {
    let name = crate::images::safe_subpath(&name).ok_or_else(|| "invalid name".to_string())?;
    let sub = crate::images::safe_subpath(&crate::images::shard(&note_id)).ok_or_else(|| "invalid note id".to_string())?;
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
    if ids.is_empty() { notes } else { notes.into_iter().filter(|n| ids.contains(&n.id)).collect() }
}

#[tauri::command]
pub fn export_notes_base64(store: State<'_, Mutex<Store>>, app: AppHandle, path: String, ids: Vec<String>) -> Result<(), String> {
    let notes = { let s = store.lock().map_err(|e| e.to_string())?; s.load_notes().map_err(|e| e.to_string())? };
    let root = crate::images::images_dir(&app);
    let out: Vec<crate::storage::Note> = select_notes(notes, &ids).into_iter().map(|mut n| {
        n.content = crate::export::inline_images(&n.content, |rel| {
            let safe = crate::images::safe_subpath(rel)?;
            let bytes = std::fs::read(root.join(&safe)).ok()?;
            Some((crate::images::mime_for(rel).to_string(), bytes))
        });
        n
    }).collect();
    let json = serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn note_inlined_html(store: State<'_, Mutex<Store>>, app: AppHandle, note_id: String) -> Result<String, String> {
    let notes = { let s = store.lock().map_err(|e| e.to_string())?; s.load_all_notes().map_err(|e| e.to_string())? };
    let note = notes.into_iter().find(|n| n.id == note_id).ok_or_else(|| "note not found".to_string())?;
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
pub fn export_md_bundle(app: AppHandle, dir: String, md: String, name: String) -> Result<(), String> {
    let (rewritten, paths) = crate::export::to_bundle(&md);
    let root = crate::images::images_dir(&app);
    let dest = std::path::PathBuf::from(&dir);
    std::fs::create_dir_all(dest.join("images")).map_err(|e| e.to_string())?;
    for rel in paths {
        if let Some(safe) = crate::images::safe_subpath(&rel) {
            let to = dest.join("images").join(&safe);
            if let Some(p) = to.parent() { let _ = std::fs::create_dir_all(p); }
            let _ = std::fs::copy(root.join(&safe), &to);
        }
    }
    let fname = format!("{}.md", name.replace(['/', '\\', ':'], "-"));
    std::fs::write(dest.join(fname), rewritten).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn export_notes_bundle(store: State<'_, Mutex<Store>>, app: AppHandle, dir: String, ids: Vec<String>) -> Result<(), String> {
    let notes = { let s = store.lock().map_err(|e| e.to_string())?; s.load_notes().map_err(|e| e.to_string())? };
    let root = crate::images::images_dir(&app);
    let dest = std::path::PathBuf::from(&dir);
    std::fs::create_dir_all(dest.join("images")).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for mut n in select_notes(notes, &ids) {
        let (content, paths) = crate::export::to_bundle(&n.content);
        for rel in paths {
            if let Some(safe) = crate::images::safe_subpath(&rel) {
                let to = dest.join("images").join(&safe);
                if let Some(parent) = to.parent() { let _ = std::fs::create_dir_all(parent); }
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
    let ctx = { let r = reg.lock().map_err(|e| e.to_string())?; active_server(&r).ok_or("no active server context")? };
    let tokens = crate::auth::load_tokens(&ctx.id)?.ok_or("not authenticated")?;
    crate::sync::fetch_workspaces(&ctx.server_url, &tokens.access_token).await.map_err(|e| e.to_string())
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
        if !label.is_empty() { r.rename(&id, label)?; }
        crate::profiles::save(&crate::config::profiles_path(&app), &r).map_err(|e| e.to_string())?;
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
        return Ok(SyncStatus { state: "local".into(), last_synced_at: 0, pending: 0 });
    };
    if ctx.workspace_id.is_empty() {
        return Ok(SyncStatus { state: "unbound".into(), last_synced_at: 0, pending: 0 });
    }
    let s = store.lock().map_err(|e| e.to_string())?;
    let last = crate::migrate::get_meta_i64(&s.conn, "sync_last_at", 0);
    let pending = s.load_dirty_notes().map_err(|e| e.to_string())?.len() as i64
        + crate::folders::load_dirty_folders(&s.conn).map_err(|e| e.to_string())?.len() as i64;
    let state = if last > 0 { "synced" } else { "syncing" };
    Ok(SyncStatus { state: state.into(), last_synced_at: last, pending })
}
