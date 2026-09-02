// src-tauri/src/widgetshare.rs
//
// Publish a snapshot of the active context into the App Group container so the
// WidgetKit extension can render it (create-new-note + pinned/recent lists).
// Best-effort; never fatal.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::storage::Note;

// Team-ID-prefixed on purpose: on macOS 15+ a `group.*` identifier without a
// provisioning profile makes the OS ask the user for consent ("wants to access
// data from other apps") on every launch — and the sandboxed widget extension,
// which has no UI to consent with, is silently denied and renders empty. A
// `<TeamID>.<name>` group is trusted from the code signature alone, for both
// the app and the widget. Must match `widget/NotefixWidget/NotefixWidget.swift`
// and both entitlements files.
const APP_GROUP: &str = "5V8ZCK434F.dev.noix.notefix";
const MAX_PINNED: usize = 6;
const MAX_RECENT: usize = 8;

/// The App Group container directory given a home directory. Pulled out of
/// [`container_dir`] so the path-joining logic is testable without depending
/// on the real home directory.
fn container_dir_in(home: &Path) -> PathBuf {
    home.join("Library/Group Containers").join(APP_GROUP)
}

fn container_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| container_dir_in(&h))
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
/// The title shown on a widget row. A protected note's `content` is
/// ciphertext, so its title must come from the plaintext `title` column
/// (deliberately unencrypted for findability) — never from the content.
fn item_title(n: &Note) -> String {
    if n.protected {
        if n.title.trim().is_empty() {
            "Geschützte Notiz".to_string()
        } else {
            n.title.clone()
        }
    } else {
        crate::tray::note_title(&n.content)
    }
}

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
            title: item_title(n),
        })
        .collect();

    let mut by_recent: Vec<&&Note> = active.iter().collect();
    by_recent.sort_by_key(|n| std::cmp::Reverse(n.updated_at));
    let recent = by_recent
        .iter()
        .take(MAX_RECENT)
        .map(|n| WidgetItem {
            id: n.id.clone(),
            title: item_title(n),
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

/// Serialize the snapshot and write it to `dir/widget.json`, creating `dir` if
/// needed. Pulled out of [`publish`] so the file/serialization logic is
/// testable against a temp directory, independent of Tauri app state.
fn write_snapshot(dir: &Path, snap: &WidgetSnapshot) -> std::io::Result<()> {
    let json = serde_json::to_string(snap)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("widget.json"), json)
}

/// Build the snapshot from the app's active store + context label, write it into
/// the App Group container, and ask WidgetKit to reload. Best-effort; never fatal.
/// True when a WidgetKit extension is embedded next to this executable
/// (`Notefix.app/Contents/MacOS/notefix` → `Contents/PlugIns/NotefixWidget.appex`).
/// Only `widget/build.sh` produces such a bundle; `tauri dev` and plain
/// `tauri build` don't, and those builds also carry no app-group entitlement.
pub fn widget_bundled_at(exe: &Path) -> bool {
    exe.parent()
        .and_then(Path::parent)
        .map(|contents| contents.join("PlugIns/NotefixWidget.appex").is_dir())
        .unwrap_or(false)
}

fn widget_bundled() -> bool {
    std::env::current_exe()
        .map(|exe| widget_bundled_at(&exe))
        .unwrap_or(false)
}

/// Publish the widget snapshot — but only when a widget is actually bundled.
/// Touching `~/Library/Group Containers/<group>` from a build without a
/// signed app-group entitlement (every dev/debug build) makes macOS prompt
/// "wants to access data from other apps" on each launch, and there is no
/// widget to feed anyway.
pub fn publish(app: &tauri::AppHandle) {
    use std::sync::Mutex;
    use tauri::Manager;
    if !widget_bundled() {
        return;
    }

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
    if let Some(dir) = container_dir() {
        let _ = write_snapshot(&dir, &snap);
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
            protected: false,
            title: String::new(),
            mcp_hidden: false,
        }
    }

    #[test]
    fn widget_bundled_only_when_the_appex_sits_in_the_bundle() {
        let root = std::env::temp_dir().join(format!("nfx-widget-{}", std::process::id()));
        let exe = root.join("Notefix.app/Contents/MacOS/notefix");
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        assert!(!widget_bundled_at(&exe), "no PlugIns → not bundled");
        std::fs::create_dir_all(root.join("Notefix.app/Contents/PlugIns/NotefixWidget.appex"))
            .unwrap();
        assert!(widget_bundled_at(&exe));
        // A bare dev executable (target/debug/notefix) has no bundle at all.
        assert!(!widget_bundled_at(Path::new("/tmp/target/debug/notefix")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn protected_notes_use_their_plaintext_title_never_the_ciphertext() {
        let mut secret = note("s1", "CIPHER:not-html-at-all", true, false, 500);
        secret.protected = true;
        secret.title = "Secret Title".into();
        let mut untitled = note("s2", "CIPHER:blob", false, false, 400);
        untitled.protected = true;
        let s = build_snapshot("Lokal", &[secret, untitled]);
        assert_eq!(s.pinned[0].title, "Secret Title");
        assert_eq!(s.recent[0].title, "Secret Title");
        assert_eq!(s.recent[1].title, "Geschützte Notiz");
        let json = serde_json::to_string(&s).unwrap();
        assert!(
            !json.contains("CIPHER"),
            "ciphertext must never reach the widget: {json}"
        );
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

    #[test]
    fn container_dir_in_joins_app_group_path() {
        let home = PathBuf::from("/Users/someone");
        assert_eq!(
            container_dir_in(&home),
            PathBuf::from("/Users/someone/Library/Group Containers/5V8ZCK434F.dev.noix.notefix")
        );
    }

    #[test]
    fn write_snapshot_creates_dir_and_writes_json() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("nested/app-group");
        let snap = build_snapshot("Lokal", &[note("x", "<p>Hi</p>", true, false, 1)]);

        write_snapshot(&dir, &snap).unwrap();

        let written = std::fs::read_to_string(dir.join("widget.json")).unwrap();
        assert!(written.contains("\"context\":\"Lokal\""), "{written}");
        assert!(written.contains("\"id\":\"x\""), "{written}");
    }

    #[test]
    fn write_snapshot_overwrites_an_existing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        write_snapshot(&dir, &build_snapshot("First", &[])).unwrap();
        write_snapshot(&dir, &build_snapshot("Second", &[])).unwrap();

        let written = std::fs::read_to_string(dir.join("widget.json")).unwrap();
        assert!(written.contains("\"context\":\"Second\""), "{written}");
    }
}
