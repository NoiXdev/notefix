use std::path::PathBuf;
use tauri::AppHandle;

pub fn images_dir(app: &AppHandle) -> PathBuf {
    let db = crate::config::read_db_path(app);
    let dir = db.parent().map(|p| p.to_path_buf()).unwrap_or_default().join("images");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn sanitize_name(name: &str) -> Option<String> {
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return None;
    }
    Some(name.to_string())
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
    fn sanitize_rejects_traversal() {
        assert_eq!(sanitize_name("abc.png").as_deref(), Some("abc.png"));
        assert!(sanitize_name("../x").is_none());
        assert!(sanitize_name("a/b").is_none());
        assert!(sanitize_name("a\\b").is_none());
        assert!(sanitize_name("").is_none());
    }
    #[test]
    fn mime_maps() {
        assert_eq!(mime_for("x.png"), "image/png");
        assert_eq!(mime_for("x.JPEG"), "image/jpeg");
        assert_eq!(mime_for("x.bin"), "application/octet-stream");
    }
}
