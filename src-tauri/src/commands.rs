use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::storage::{Note, Store};

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
        store.delete_note(&id).map_err(|e| e.to_string())?;
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
    { let store = store.lock().map_err(|e| e.to_string())?; crate::folders::delete_folder(&store.conn, &id, crate::folders::DeleteMode::from_str(&mode)).map_err(|e| e.to_string())?; }
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
