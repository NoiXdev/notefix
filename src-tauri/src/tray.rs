use std::sync::Mutex;

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};

use crate::storage::{Note, Store};

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
pub(crate) fn note_title(html: &str) -> String {
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

/// The tray menu-item id and display label for each recent note, in order.
/// Pulled out of `build_menu` so this menu-model construction logic is
/// testable without a running Tauri app.
fn recent_menu_entries(notes: Vec<Note>) -> Vec<(String, String)> {
    notes
        .into_iter()
        .map(|n| (format!("tray_open:{}", n.id), note_title(&n.content)))
        .collect()
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let new = MenuItem::with_id(app, "tray_new", "Neue Notiz", true, None::<&str>)?;
    let open_last = MenuItem::with_id(
        app,
        "tray_open_last",
        "Letzte Notiz öffnen",
        true,
        None::<&str>,
    )?;

    let recent = Submenu::with_id(app, "tray_recent", "Zuletzt geöffnet", true)?;
    let notes = app
        .state::<Mutex<Store>>()
        .lock()
        .ok()
        .and_then(|s| s.recent_notes(5).ok())
        .unwrap_or_default();
    let entries = recent_menu_entries(notes);
    if entries.is_empty() {
        let empty = MenuItem::with_id(
            app,
            "tray_recent_empty",
            "(keine Notizen)",
            false,
            None::<&str>,
        )?;
        recent.append(&empty)?;
    } else {
        for (id, label) in entries {
            let item = MenuItem::with_id(app, id, label, true, None::<&str>)?;
            recent.append(&item)?;
        }
    }

    let toggle = MenuItem::with_id(
        app,
        "tray_toggle",
        "Fenster zeigen/verstecken",
        true,
        None::<&str>,
    )?;
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

/// Pick the id of the first (most recent) note. Pulled out of
/// [`last_note_id`] so this bookkeeping is testable without a Tauri app/DB.
fn first_note_id(notes: Vec<Note>) -> Option<String> {
    notes.into_iter().next().map(|n| n.id)
}

fn last_note_id(app: &AppHandle) -> Option<String> {
    app.state::<Mutex<Store>>()
        .lock()
        .ok()
        .and_then(|s| s.recent_notes(1).ok())
        .and_then(first_note_id)
}

/// The semantic action for a tray menu-item id. Pulled out of
/// [`handle_menu_event`] so the id -> action mapping is testable without a
/// running Tauri app.
#[derive(Debug, PartialEq, Eq)]
enum MenuAction {
    NewNote,
    OpenLast,
    Toggle,
    OpenSettings,
    Quit,
    OpenNote(String),
    Unknown,
}

fn classify_menu_event(id: &str) -> MenuAction {
    match id {
        "tray_new" => MenuAction::NewNote,
        "tray_open_last" => MenuAction::OpenLast,
        "tray_toggle" => MenuAction::Toggle,
        "tray_settings" => MenuAction::OpenSettings,
        "tray_quit" => MenuAction::Quit,
        other => match other.strip_prefix("tray_open:") {
            Some(note_id) => MenuAction::OpenNote(note_id.to_string()),
            None => MenuAction::Unknown,
        },
    }
}

fn handle_menu_event(app: &AppHandle, id: &str) {
    match classify_menu_event(id) {
        MenuAction::NewNote => {
            show_main(app);
            let _ = app.emit("tray://new-note", ());
        }
        MenuAction::OpenLast => {
            show_main(app);
            if let Some(id) = last_note_id(app) {
                let _ = app.emit("tray://open-note", id);
            }
        }
        MenuAction::Toggle => {
            if let Some(w) = app.get_webview_window("main") {
                if w.is_visible().unwrap_or(false) {
                    let _ = w.hide();
                } else {
                    show_main(app);
                }
            }
        }
        MenuAction::OpenSettings => {
            show_main(app);
            let _ = app.emit("tray://open-settings", ());
        }
        MenuAction::Quit => app.exit(0),
        MenuAction::OpenNote(note_id) => {
            show_main(app);
            let _ = app.emit("tray://open-note", note_id);
        }
        MenuAction::Unknown => {}
    }
}

pub fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(
            app.default_window_icon()
                .cloned()
                .expect("app has a default icon"),
        )
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
    // Notes/context changed → refresh the widget snapshot too. This is the
    // single republish hook: rebuild_menu is called after every note mutation
    // and from broadcast_context_changed (context switch / sync apply).
    crate::widgetshare::publish(app);
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn recent_menu_entries_builds_id_and_title_pairs_in_order() {
        let notes = vec![
            Note {
                id: "n1".into(),
                content: "<p>Hello</p>".into(),
                ..Default::default()
            },
            Note {
                id: "n2".into(),
                content: "<p>World</p>".into(),
                ..Default::default()
            },
        ];
        assert_eq!(
            recent_menu_entries(notes),
            vec![
                ("tray_open:n1".to_string(), "Hello".to_string()),
                ("tray_open:n2".to_string(), "World".to_string()),
            ]
        );
    }

    #[test]
    fn recent_menu_entries_empty_when_no_notes() {
        assert!(recent_menu_entries(vec![]).is_empty());
    }

    #[test]
    fn first_note_id_returns_first_or_none() {
        assert_eq!(first_note_id(vec![]), None);
        let notes = vec![
            Note {
                id: "n1".into(),
                ..Default::default()
            },
            Note {
                id: "n2".into(),
                ..Default::default()
            },
        ];
        assert_eq!(first_note_id(notes), Some("n1".to_string()));
    }

    #[test]
    fn classify_menu_event_maps_known_ids() {
        assert_eq!(classify_menu_event("tray_new"), MenuAction::NewNote);
        assert_eq!(classify_menu_event("tray_open_last"), MenuAction::OpenLast);
        assert_eq!(classify_menu_event("tray_toggle"), MenuAction::Toggle);
        assert_eq!(
            classify_menu_event("tray_settings"),
            MenuAction::OpenSettings
        );
        assert_eq!(classify_menu_event("tray_quit"), MenuAction::Quit);
    }

    #[test]
    fn classify_menu_event_extracts_open_note_id() {
        assert_eq!(
            classify_menu_event("tray_open:abc123"),
            MenuAction::OpenNote("abc123".to_string())
        );
    }

    #[test]
    fn classify_menu_event_unknown_id_is_unknown() {
        assert_eq!(classify_menu_event("something_else"), MenuAction::Unknown);
        assert_eq!(classify_menu_event(""), MenuAction::Unknown);
    }
}
