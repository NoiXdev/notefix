use std::path::PathBuf;
use tauri::AppHandle;

pub fn images_dir(app: &AppHandle) -> PathBuf {
    let db = crate::config::read_db_path(app);
    let dir = db.parent().map(|p| p.to_path_buf()).unwrap_or_default().join("images");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Note-ID (UUID) → verschachtelter Pfad: jeder '-' wird ein Verzeichnistrenner.
pub fn shard(id: &str) -> String {
    id.replace('-', "/")
}

/// Validiert einen images-relativen Subpfad: erlaubt `a/b/c.ext`, lehnt
/// leere/`.`/`..`/absolute/Backslash-Segmente ab. Gibt den normalisierten Pfad zurück.
pub fn safe_subpath(path: &str) -> Option<String> {
    if path.is_empty() { return None; }
    let mut parts = Vec::new();
    for seg in path.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." || seg.contains('\\') {
            return None;
        }
        parts.push(seg);
    }
    if parts.is_empty() { return None; }
    Some(parts.join("/"))
}

/// Sharded noteimg-URL für ein Bild einer Notiz.
pub fn note_image_url(note_id: &str, name: &str) -> String {
    format!("noteimg://localhost/{}/{}", shard(note_id), name)
}

pub fn mime_for(name: &str) -> &'static str {
    let l = name.to_lowercase();
    if l.ends_with(".png") { "image/png" }
    else if l.ends_with(".jpg") || l.ends_with(".jpeg") { "image/jpeg" }
    else if l.ends_with(".gif") { "image/gif" }
    else if l.ends_with(".webp") { "image/webp" }
    else if l.ends_with(".svg") { "image/svg+xml" }
    else { "application/octet-stream" }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shard_splits_uuid_on_hyphens() {
        assert_eq!(shard("5061b1e2-bad1-4fc7-a4f6-e16577f5dca4"), "5061b1e2/bad1/4fc7/a4f6/e16577f5dca4");
        assert_eq!(shard("a"), "a");
    }
    #[test]
    fn safe_subpath_accepts_nested_rejects_traversal() {
        assert_eq!(safe_subpath("a/b/c.png").as_deref(), Some("a/b/c.png"));
        assert_eq!(safe_subpath("x.png").as_deref(), Some("x.png"));
        assert!(safe_subpath("../x").is_none());
        assert!(safe_subpath("a/../b").is_none());
        assert!(safe_subpath("/a").is_none());
        assert!(safe_subpath("a/").is_none());
        assert!(safe_subpath("").is_none());
        assert!(safe_subpath("a\\b").is_none());
    }
    #[test]
    fn mime_maps() {
        assert_eq!(mime_for("x.png"), "image/png");
        assert_eq!(mime_for("a/b/x.JPEG"), "image/jpeg");
        assert_eq!(mime_for("x.bin"), "application/octet-stream");
    }
    #[test]
    fn note_image_url_builds_sharded_url() {
        assert_eq!(note_image_url("5061b1e2-bad1-4fc7-a4f6-e16577f5dca4", "x.png"),
            "noteimg://localhost/5061b1e2/bad1/4fc7/a4f6/e16577f5dca4/x.png");
    }
}
