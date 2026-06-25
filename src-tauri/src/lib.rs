mod auth;
mod commands;
mod config;
mod export;
mod folders;
mod images;
mod linkmeta;
mod mcp;
mod migrate;
mod profiles;
mod revisions;
mod settings;
mod stats;
mod storage;
mod sync;
mod syscheck;
mod tray;

use std::sync::Mutex;

use tauri::Emitter;
use tauri::Manager;

use storage::Store;

fn legacy_notes_dir() -> Option<std::path::PathBuf> {
    dirs::data_dir().map(|d| d.join("dginxNotes").join("notes"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .register_uri_scheme_protocol("noteimg", |ctx, request| {
            let app = ctx.app_handle();
            let name = request.uri().path().trim_start_matches('/').to_string();
            let body = images::safe_subpath(&name)
                .and_then(|n| std::fs::read(images::images_dir(app).join(n)).ok());
            match body {
                Some(bytes) => tauri::http::Response::builder()
                    .header("Content-Type", images::mime_for(&name))
                    .body(bytes)
                    .unwrap(),
                None => tauri::http::Response::builder().status(404).body(Vec::new()).unwrap(),
            }
        })
        // single-instance MUST be registered first. On Windows/Linux a
        // `notefix://…` deep link arrives as an argv of the second instance;
        // forward it to the running window as an `auth-callback` event.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            tray::show_main(app);
            if let Some(url) = args.iter().find(|a| a.starts_with("notefix://")) {
                let _ = app.emit("auth-callback", url.clone());
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Deep-link auth bridge: when the OS hands the running app a
            // `notefix://…` URL (browser sign-in redirect), forward it to the
            // frontend as an `auth-callback` event. On Linux/Windows dev builds
            // the scheme also has to be registered at runtime.
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                #[cfg(any(target_os = "linux", windows))]
                let _ = app.deep_link().register_all();
                let handle = app.handle().clone();
                app.deep_link().on_open_url(move |event| {
                    for url in event.urls() {
                        tray::show_main(&handle);
                        let _ = handle.emit("auth-callback", url.to_string());
                    }
                });
            }

            // Seed a registry from the existing single-DB path so existing
            // users keep their database, then open the active context's DB.
            let default_db = config::read_db_path(app.handle());
            let prof_path = config::profiles_path(app.handle());
            let mut reg = profiles::load(&prof_path, &default_db.to_string_lossy());
            // Relocate legacy flat context DBs (<contexts>/<id>.db) into per-context
            // subdirectories (<contexts>/<id>/notefix.db) so each context owns its
            // own images folder (resolved as <db-dir>/images).
            {
                let cdir = config::contexts_dir(app.handle());
                for c in reg.contexts.iter_mut() {
                    let old = std::path::PathBuf::from(&c.path);
                    if profiles::is_flat_context_path(&old, &cdir) {
                        let new_dir = cdir.join(&c.id);
                        let _ = std::fs::create_dir_all(&new_dir);
                        let new_db = new_dir.join("notefix.db");
                        for ext in ["", "-wal", "-shm"] {
                            let from = std::path::PathBuf::from(format!("{}{}", old.to_string_lossy(), ext));
                            let to = std::path::PathBuf::from(format!("{}{}", new_db.to_string_lossy(), ext));
                            if from.exists() { let _ = std::fs::rename(&from, &to); }
                        }
                        c.path = new_db.to_string_lossy().into_owned();
                    }
                }
            }
            // First run / upgrade / post-migration: persist the registry.
            let _ = profiles::save(&prof_path, &reg);

            let db_path = std::path::PathBuf::from(&reg.active().expect("active context").path);
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            // The active context's images folder. Computed directly from db_path
            // because the registry isn't managed yet, so images_dir(app) would
            // fall back to the default DB here and GC the wrong folder.
            let active_images = images::images_root_for(&db_path);
            let _ = std::fs::create_dir_all(&active_images);
            let mut store = Store::open(&db_path)?;
            migrate::run_migrations(&store.conn)?;
            store.sync_enabled = reg.active().map(|c| c.kind == "server").unwrap_or(false);
            {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
                let days = settings::get_int(&store.conn, "trashRetentionDays", 30);
                let _ = store.purge_trashed(Some(now - days * 86_400_000));
            }
            if !settings::get_bool(&store.conn, "imagesMigrated") {
                let _ = images::migrate_inline_images(&store, &active_images);
                let _ = settings::set_setting(&store.conn, "imagesMigrated", "true");
            }
            let _ = images::gc_images(&active_images, &images::collect_referenced(&store));
            if let Some(legacy) = legacy_notes_dir() {
                let _ = migrate::import_legacy_if_needed(&store, &legacy);
            }
            let start_minimized = settings::get_bool(&store.conn, "startMinimized");

            // Read MCP settings before `store` is moved into the managed state.
            let mcp_enabled = settings::get_bool(&store.conn, "mcpEnabled");
            let mcp_bind = settings::get_string(&store.conn, "mcpBind", "internal");
            let mcp_port = settings::get_int(&store.conn, "mcpPort", 4357) as u16;
            let mcp_token = settings::get_string(&store.conn, "mcpToken", "");
            let mcp_auth_required = settings::get_bool_default(&store.conn, "mcpAuthRequired", true);
            let mcp_allow_write = settings::get_bool(&store.conn, "mcpAllowWrite");

            app.manage(Mutex::new(store));
            app.manage(Mutex::new(reg));
            // In-memory store of in-flight browser auth flows (A1).
            app.manage(commands::PendingAuthMap::default());
            // Notify the background sync loop (Task 7) when an edit or manual
            // trigger wants an out-of-cycle sync.
            app.manage(std::sync::Arc::new(tokio::sync::Notify::new()));

            if mcp_enabled {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    let _ = crate::mcp::apply(
                        handle,
                        true,
                        mcp_bind,
                        mcp_port,
                        mcp_token,
                        mcp_auth_required,
                        mcp_allow_write,
                    )
                    .await;
                });
            }

            tray::build_tray(app.handle())?;

            if !start_minimized {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Main window: honor the closeAction setting (ask / minimize / quit).
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    let action = {
                        let state = window.app_handle().state::<Mutex<Store>>();
                        let store = state.lock().unwrap();
                        settings::get_string(&store.conn, "closeAction", "ask")
                    };
                    match action.as_str() {
                        "quit" => { window.app_handle().exit(0); }
                        "minimize" => { api.prevent_close(); let _ = window.hide(); }
                        _ => { api.prevent_close(); let _ = window.emit("close-requested", ()); }
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::notes_load,
            commands::notes_save,
            commands::notes_delete,
            commands::notes_restore,
            commands::notes_purge,
            commands::trash_load,
            commands::trash_empty,
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
            commands::folder_set_sort,
            commands::get_db_path,
            commands::set_db_location,
            commands::open_note_window,
            commands::settings_load,
            commands::settings_set,
            commands::note_revisions,
            commands::note_revision_content,
            commands::quit_app,
            commands::hide_main,
            commands::save_image,
            commands::export_notes_base64,
            commands::export_notes_bundle,
            commands::note_inlined_html,
            commands::save_export,
            commands::export_md_bundle,
            commands::check_paths,
            commands::mcp_apply_config,
            commands::contexts_list,
            commands::context_add,
            commands::context_switch,
            commands::context_rename,
            commands::context_remove,
            commands::server_auth_begin,
            commands::server_auth_complete,
            commands::server_workspaces,
            commands::context_bind_workspace,
            commands::sync_now,
            commands::sync_status,
            linkmeta::fetch_link_meta,
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
