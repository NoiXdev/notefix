mod commands;
mod config;
mod export;
mod folders;
mod migrate;
mod revisions;
mod settings;
mod stats;
mod storage;
mod tray;

use std::sync::Mutex;

use tauri::Manager;

use storage::Store;

fn legacy_notes_dir() -> Option<std::path::PathBuf> {
    dirs::data_dir().map(|d| d.join("dginxNotes").join("notes"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // single-instance MUST be registered first.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let db_path = config::read_db_path(app.handle());
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let store = Store::open(&db_path)?;
            migrate::run_migrations(&store.conn)?;
            if let Some(legacy) = legacy_notes_dir() {
                let _ = migrate::import_legacy_if_needed(&store, &legacy);
            }
            let start_minimized = settings::get_bool(&store.conn, "startMinimized");
            app.manage(Mutex::new(store));

            tray::build_tray(app.handle())?;

            if !start_minimized {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // The main window hides instead of closing; note windows close normally.
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::notes_load,
            commands::notes_save,
            commands::notes_delete,
            commands::notes_set_pinned,
            commands::notes_set_archived,
            commands::notes_set_color,
            commands::export_notes,
            commands::notes_set_due,
            commands::note_stats,
            commands::folders_load,
            commands::folder_create,
            commands::folder_rename,
            commands::folder_move,
            commands::folder_delete,
            commands::notes_set_folder,
            commands::notes_reorder,
            commands::folders_reorder,
            commands::folder_set_icon,
            commands::folder_set_color,
            commands::get_db_path,
            commands::set_db_location,
            commands::open_note_window,
            commands::settings_load,
            commands::settings_set,
            commands::note_revisions,
            commands::note_revision_content,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // macOS: clicking the dock icon when no window is visible re-shows it.
            if let tauri::RunEvent::Reopen { .. } = event {
                tray::show_main(app);
            }
        });
}
