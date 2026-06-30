// src-tauri/src/widgetshare.rs
//
// Publish a snapshot of the active context into the App Group container so the
// WidgetKit extension can render it (create-new-note + pinned/recent lists).
// Best-effort; never fatal.

use std::path::PathBuf;

use serde::Serialize;

use crate::storage::Note;

const APP_GROUP: &str = "group.dev.noidee.notefix";
const MAX_PINNED: usize = 6;
const MAX_RECENT: usize = 8;

fn container_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join("Library/Group Containers").join(APP_GROUP))
}

#[derive(Serialize)]
pub struct WidgetItem {
    pub id: String,
    pub title: String,
}

#[derive(Serialize)]
pub struct WidgetSnapshot {
    pub context: String,
    pub count: usize,
    pub pinned: Vec<WidgetItem>,
    pub recent: Vec<WidgetItem>,
}

/// Build the widget snapshot from the active context's notes (as returned by
/// `Store::load_notes` — non-deleted, ordered pinned DESC, position ASC).
/// Pinned keeps list order (≤6); recent is newest-first by `updated_at` (≤8);
/// both exclude archived. `count` = active (non-archived, non-deleted) notes.
pub fn build_snapshot(label: &str, notes: &[Note]) -> WidgetSnapshot {
    let context = if label.trim().is_empty() {
        "Lokal".to_string()
    } else {
        label.to_string()
    };
    let active: Vec<&Note> = notes.iter().filter(|n| !n.archived).collect();

    let pinned = active
        .iter()
        .filter(|n| n.pinned)
        .take(MAX_PINNED)
        .map(|n| WidgetItem {
            id: n.id.clone(),
            title: crate::tray::note_title(&n.content),
        })
        .collect();

    let mut by_recent: Vec<&&Note> = active.iter().collect();
    by_recent.sort_by_key(|n| std::cmp::Reverse(n.updated_at));
    let recent = by_recent
        .iter()
        .take(MAX_RECENT)
        .map(|n| WidgetItem {
            id: n.id.clone(),
            title: crate::tray::note_title(&n.content),
        })
        .collect();

    WidgetSnapshot {
        context,
        count: active.len(),
        pinned,
        recent,
    }
}

// Swift shim (build.rs): WidgetCenter.shared.reloadAllTimelines(), so the widget
// re-reads the snapshot we just wrote instead of showing a stale cached timeline.
#[cfg(target_os = "macos")]
extern "C" {
    fn notefix_reload_widgets();
}

/// Ask WidgetKit to reload this app's widget timelines (macOS only).
pub fn reload_widgets() {
    #[cfg(target_os = "macos")]
    unsafe {
        notefix_reload_widgets();
    }
}

/// Build the snapshot from the app's active store + context label, write it into
/// the App Group container, and ask WidgetKit to reload. Best-effort; never fatal.
pub fn publish(app: &tauri::AppHandle) {
    use std::sync::Mutex;
    use tauri::Manager;

    let label = app
        .state::<Mutex<crate::profiles::Registry>>()
        .lock()
        .ok()
        .and_then(|r| r.active().map(|c| c.label.clone()))
        .unwrap_or_default();
    let notes = match app
        .state::<Mutex<crate::storage::Store>>()
        .lock()
        .ok()
        .and_then(|s| s.load_notes().ok())
    {
        Some(n) => n,
        None => return,
    };

    let snap = build_snapshot(&label, &notes);
    if let (Some(dir), Ok(json)) = (container_dir(), serde_json::to_string(&snap)) {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("widget.json"), json);
    }
    reload_widgets();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Note;

    fn note(id: &str, content: &str, pinned: bool, archived: bool, updated_at: i64) -> Note {
        Note {
            id: id.into(),
            content: content.into(),
            updated_at,
            pinned,
            archived,
            color: String::new(),
            due_at: None,
            folder_id: None,
            position: 0,
            deleted_at: None,
            dirty: false,
        }
    }

    #[test]
    fn snapshot_splits_pinned_and_recent_and_counts() {
        let notes = vec![
            note("p1", "<p>Pinned A</p>", true, false, 100),
            note("r1", "<p>Recent A</p>", false, false, 300),
            note("r2", "<p>Recent B</p>", false, false, 200),
            note("a1", "<p>Archived</p>", false, true, 400),
        ];
        let s = build_snapshot("Lokal", &notes);
        assert_eq!(s.context, "Lokal");
        assert_eq!(s.count, 3); // archived excluded
        assert_eq!(
            s.pinned.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
            ["p1"]
        );
        // recent newest-first, archived excluded (pinned note still appears in recent)
        assert_eq!(
            s.recent.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
            ["r1", "r2", "p1"]
        );
        assert_eq!(s.pinned[0].title, "Pinned A");
    }

    #[test]
    fn snapshot_caps_pinned_at_6_and_recent_at_8() {
        let mut notes = Vec::new();
        for i in 0..10 {
            notes.push(note(&format!("p{i}"), "<p>x</p>", true, false, i));
        }
        for i in 0..10 {
            notes.push(note(&format!("r{i}"), "<p>y</p>", false, false, 100 + i));
        }
        let s = build_snapshot("W", &notes);
        assert_eq!(s.pinned.len(), 6);
        assert_eq!(s.recent.len(), 8);
    }

    #[test]
    fn snapshot_serializes_to_expected_shape() {
        let s = build_snapshot("Lokal", &[note("x", "<p>Hi</p>", true, false, 1)]);
        let j = serde_json::to_string(&s).unwrap();
        assert!(j.contains("\"context\":\"Lokal\""), "{j}");
        assert!(j.contains("\"count\":1"), "{j}");
        assert!(
            j.contains("\"pinned\":[{\"id\":\"x\",\"title\":\"Hi\"}]"),
            "{j}"
        );
        assert!(j.contains("\"recent\":["), "{j}");
    }

    #[test]
    fn empty_label_falls_back_to_lokal() {
        assert_eq!(build_snapshot("", &[]).context, "Lokal");
    }
}
