mod commands;
mod migrate;
mod settings;
mod storage;

use std::sync::Mutex;

use tauri::Manager;

use storage::Store;

/// Standard dginx-notes user-data notes dir (macOS-correct; no-op elsewhere if absent).
fn legacy_notes_dir() -> Option<std::path::PathBuf> {
    dirs::data_dir().map(|d| d.join("dginxNotes").join("notes"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let store = Store::open(&dir.join("notefix.db"))?;
            migrate::run_migrations(&store.conn)?;
            if let Some(legacy) = legacy_notes_dir() {
                let _ = migrate::import_legacy_if_needed(&store, &legacy);
            }
            app.manage(Mutex::new(store));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::notes_load,
            commands::notes_save,
            commands::notes_delete,
            commands::notes_set_pinned,
            commands::open_note_window,
            commands::settings_load,
            commands::settings_set,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
