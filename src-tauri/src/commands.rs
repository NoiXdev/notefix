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
        if crate::settings::get_bool_default(&store.conn, "trashEnabled", true) {
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
    { let store = store.lock().map_err(|e| e.to_string())?; crate::folders::create_folder(&store.conn, &id, &name, parent_id.as_deref()).map_err(|e| e.to_string())?; }
    notify(&app, &webview);
    Ok(())
}

#[tauri::command]
pub fn folder_rename(app: AppHandle, webview: WebviewWindow, store: State<'_, Mutex<Store>>, id: String, name: String) -> Result<(), String> {
    { let store = store.lock().map_err(|e| e.to_string())?; crate::folders::rename_folder(&store.conn, &id, &name).map_err(|e| e.to_string())?; }
    notify(&app, &webview);
    Ok(())
}

#[tauri::command]
pub fn folder_move(app: AppHandle, webview: WebviewWindow, store: State<'_, Mutex<Store>>, id: String, parent_id: Option<String>) -> Result<(), String> {
    { let store = store.lock().map_err(|e| e.to_string())?; crate::folders::move_folder(&store.conn, &id, parent_id.as_deref()).map_err(|e| e.to_string())?; }
    notify(&app, &webview);
    Ok(())
}

#[tauri::command]
pub fn folder_delete(app: AppHandle, webview: WebviewWindow, store: State<'_, Mutex<Store>>, id: String, mode: String) -> Result<(), String> {
    { let store = store.lock().map_err(|e| e.to_string())?; crate::folders::delete_folder(&store.conn, &id, crate::folders::DeleteMode::from_str(&mode)).map_err(|e| e.to_string())?; crate::images::run_gc(&app, &store); }
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
    { let store = store.lock().map_err(|e| e.to_string())?; crate::folders::set_folder_icon(&store.conn, &id, &icon).map_err(|e| e.to_string())?; }
    notify(&app, &webview);
    Ok(())
}

#[tauri::command]
pub fn folder_set_color(app: AppHandle, webview: WebviewWindow, store: State<'_, Mutex<Store>>, id: String, color: String) -> Result<(), String> {
    { let store = store.lock().map_err(|e| e.to_string())?; crate::folders::set_folder_color(&store.conn, &id, &color).map_err(|e| e.to_string())?; }
    notify(&app, &webview);
    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbLocationResult {
    pub mode: String,
    pub path: String,
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
    { let store = store.lock().map_err(|e| e.to_string())?; crate::folders::set_folder_sort(&store.conn, &id, &sort).map_err(|e| e.to_string())?; }
    notify(&app, &webview);
    Ok(())
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
