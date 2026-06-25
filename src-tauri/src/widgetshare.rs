// src-tauri/src/widgetshare.rs
//
// Widget spike: write a tiny snapshot into the App Group container so the
// WidgetKit extension can read it. Best-effort; never fatal.

use std::path::PathBuf;

const APP_GROUP: &str = "group.dev.noidee.notefix";

fn container_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join("Library/Group Containers").join(APP_GROUP))
}

/// The exact JSON the widget reads ({"hello":"from app <ts>"}).
pub fn hello_json(ts: i64) -> String {
    format!("{{\"hello\":\"from app {ts}\"}}")
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

/// Write the snapshot into the App Group container (creating the dir), then ask
/// the widget to refresh.
pub fn write_hello() {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    if let Some(dir) = container_dir() {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("widget.json"), hello_json(ts));
    }
    reload_widgets();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_json_has_expected_shape() {
        assert_eq!(hello_json(123), "{\"hello\":\"from app 123\"}");
    }

    #[test]
    fn container_dir_is_the_group_path() {
        let d = container_dir().expect("home dir");
        assert!(d.ends_with("Library/Group Containers/group.dev.noidee.notefix"), "got {d:?}");
    }
}
