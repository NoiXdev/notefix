use std::sync::Mutex;

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};

use crate::storage::Store;

pub const TRAY_ID: &str = "main-tray";

/// Show + focus the main window (used by tray actions, dock reopen, single-instance).
pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Plain-text, single-line, length-capped label from a note's HTML content.
fn note_title(html: &str) -> String {
    let mut text = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => {
                in_tag = true;
                if !text.is_empty() && !text.ends_with(' ') {
                    text.push(' ');
                }
            }
            '>' => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return "Neue Notiz".to_string();
    }
    if trimmed.chars().count() > 40 {
        format!("{}…", trimmed.chars().take(40).collect::<String>())
    } else {
        trimmed.to_string()
    }
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let new = MenuItem::with_id(app, "tray_new", "Neue Notiz", true, None::<&str>)?;
    let open_last = MenuItem::with_id(app, "tray_open_last", "Letzte Notiz öffnen", true, None::<&str>)?;

    let recent = Submenu::with_id(app, "tray_recent", "Zuletzt geöffnet", true)?;
    let notes = app
        .state::<Mutex<Store>>()
        .lock()
        .ok()
        .and_then(|s| s.recent_notes(5).ok())
        .unwrap_or_default();
    if notes.is_empty() {
        let empty = MenuItem::with_id(app, "tray_recent_empty", "(keine Notizen)", false, None::<&str>)?;
        recent.append(&empty)?;
    } else {
        for n in notes {
            let item = MenuItem::with_id(app, format!("tray_open:{}", n.id), note_title(&n.content), true, None::<&str>)?;
            recent.append(&item)?;
        }
    }

    let toggle = MenuItem::with_id(app, "tray_toggle", "Fenster zeigen/verstecken", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "tray_settings", "Einstellungen", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "tray_quit", "Beenden", true, None::<&str>)?;

    Menu::with_items(
        app,
        &[
            &new,
            &open_last,
            &recent,
            &PredefinedMenuItem::separator(app)?,
            &toggle,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )
}

fn last_note_id(app: &AppHandle) -> Option<String> {
    app.state::<Mutex<Store>>()
        .lock()
        .ok()
        .and_then(|s| s.recent_notes(1).ok())
        .and_then(|v| v.into_iter().next())
        .map(|n| n.id)
}

fn handle_menu_event(app: &AppHandle, id: &str) {
    match id {
        "tray_new" => {
            show_main(app);
            let _ = app.emit("tray://new-note", ());
        }
        "tray_open_last" => {
            show_main(app);
            if let Some(id) = last_note_id(app) {
                let _ = app.emit("tray://open-note", id);
            }
        }
        "tray_toggle" => {
            if let Some(w) = app.get_webview_window("main") {
                if w.is_visible().unwrap_or(false) {
                    let _ = w.hide();
                } else {
                    show_main(app);
                }
            }
        }
        "tray_settings" => {
            show_main(app);
            let _ = app.emit("tray://open-settings", ());
        }
        "tray_quit" => app.exit(0),
        other => {
            if let Some(note_id) = other.strip_prefix("tray_open:") {
                show_main(app);
                let _ = app.emit("tray://open-note", note_id.to_string());
            }
        }
    }
}

pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().cloned().expect("app has a default icon"))
        .icon_as_template(false)
        .tooltip("Notefix")
        .menu(&menu)
        .on_menu_event(|app, event| handle_menu_event(app, event.id.as_ref()))
        .build(app)?;
    Ok(())
}

/// Rebuild the tray menu (e.g. after notes change, to refresh the "recent" submenu).
pub fn rebuild_menu(app: &AppHandle) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        if let Ok(menu) = build_menu(app) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::note_title;

    #[test]
    fn note_title_strips_html_and_separates_elements() {
        assert_eq!(note_title("<h1>Titel</h1><p>Rest</p>"), "Titel Rest");
    }

    #[test]
    fn note_title_falls_back_for_empty() {
        assert_eq!(note_title("<p></p>"), "Neue Notiz");
    }

    #[test]
    fn note_title_truncates_long_text() {
        let long = format!("<p>{}</p>", "a".repeat(60));
        let t = note_title(&long);
        assert!(t.ends_with('…'));
        assert_eq!(t.chars().count(), 41);
    }
}
