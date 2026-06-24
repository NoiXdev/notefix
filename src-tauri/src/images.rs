use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

use base64::Engine;

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

fn ext_for_mime(mime: &str) -> &'static str {
    match mime.to_lowercase().as_str() {
        "png" => "png",
        "jpeg" | "jpg" => "jpeg",
        "gif" => "gif",
        "webp" => "webp",
        "svg+xml" => "svg",
        _ => "png",
    }
}

/// Findet `data:image/<typ>;base64,<daten>`-srcs und ersetzt jede durch eine
/// sharded noteimg-URL. Gibt (neuer_content, zu_schreibende_dateien) zurück;
/// jede Datei = (relativer_pfad_unter_images, bytes).
pub fn rewrite_data_urls(content: &str, note_id: &str) -> (String, Vec<(String, Vec<u8>)>) {
    let re = regex::Regex::new(r#"data:image/([a-zA-Z0-9.+-]+);base64,([A-Za-z0-9+/=]+)"#).unwrap();
    let shard = shard(note_id);
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let new = re
        .replace_all(content, |caps: &regex::Captures| {
            match base64::engine::general_purpose::STANDARD.decode(&caps[2]) {
                Ok(bytes) => {
                    let name = format!("{}.{}", uuid::Uuid::new_v4(), ext_for_mime(&caps[1]));
                    let rel = format!("{shard}/{name}");
                    let url = format!("noteimg://localhost/{rel}");
                    files.push((rel, bytes));
                    url
                }
                Err(_) => caps[0].to_string(),
            }
        })
        .to_string();
    (new, files)
}

/// Lagert alle base64-Bilder aller Notizen als Dateien aus (best effort).
pub fn migrate_inline_images(store: &crate::storage::Store, images_root: &std::path::Path) -> Result<(), String> {
    for note in store.load_all_notes().map_err(|e| e.to_string())? {
        if !note.content.contains("data:image/") { continue; }
        let (new_content, files) = rewrite_data_urls(&note.content, &note.id);
        if files.is_empty() { continue; }
        for (rel, bytes) in files {
            if let Some(safe) = safe_subpath(&rel) {
                let path = images_root.join(&safe);
                if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
                std::fs::write(path, bytes).map_err(|e| e.to_string())?;
            }
        }
        store.set_content_silent(&note.id, &new_content).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Alle in einem Content referenzierten noteimg-Relativpfade.
pub fn referenced_paths(content: &str) -> Vec<String> {
    let re = regex::Regex::new(r#"noteimg://localhost/([^"'\s\\)]+)"#).unwrap();
    re.captures_iter(content).map(|c| c[1].to_string()).collect()
}

/// Set aller referenzierten Pfade über alle Notizen (inkl. Papierkorb).
pub fn collect_referenced(store: &crate::storage::Store) -> HashSet<String> {
    let mut set = HashSet::new();
    if let Ok(notes) = store.load_all_notes() {
        for n in notes { for p in referenced_paths(&n.content) { set.insert(p); } }
    }
    set
}

fn prune_empty_dirs(dir: &Path, root: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() { prune_empty_dirs(&p, root); }
        }
    }
    if dir != root {
        if let Ok(mut it) = std::fs::read_dir(dir) {
            if it.next().is_none() { let _ = std::fs::remove_dir(dir); }
        }
    }
}

/// Löscht alle Dateien unter `images_root`, deren Relativpfad nicht referenziert ist.
pub fn gc_images(images_root: &Path, referenced: &HashSet<String>) -> std::io::Result<()> {
    if !images_root.exists() { return Ok(()); }
    let mut stack = vec![images_root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let p = entry?.path();
            if p.is_dir() { stack.push(p); } else { files.push(p); }
        }
    }
    for f in &files {
        if let Ok(rel) = f.strip_prefix(images_root) {
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            if !referenced.contains(&rel_str) { let _ = std::fs::remove_file(f); }
        }
    }
    prune_empty_dirs(images_root, images_root);
    Ok(())
}

/// Convenience: aktuelle Referenzen sammeln und GC ausführen (best effort).
pub fn run_gc(app: &AppHandle, store: &crate::storage::Store) {
    let referenced = collect_referenced(store);
    let _ = gc_images(&images_dir(app), &referenced);
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
    #[test]
    fn rewrite_data_urls_extracts_one_png() {
        // 1x1 transparentes PNG (base64)
        let b64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
        let content = format!("<p><img src=\"data:image/png;base64,{b64}\"></p>");
        let (new, files) = rewrite_data_urls(&content, "5061b1e2-bad1-4fc7-a4f6-e16577f5dca4");
        assert!(!new.contains("data:image"));
        assert!(new.contains("noteimg://localhost/5061b1e2/bad1/4fc7/a4f6/e16577f5dca4/"));
        assert_eq!(files.len(), 1);
        assert!(files[0].0.starts_with("5061b1e2/bad1/4fc7/a4f6/e16577f5dca4/"));
        assert!(files[0].0.ends_with(".png"));
        assert!(!files[0].1.is_empty());
    }
    #[test]
    fn rewrite_data_urls_noop_without_images() {
        let (new, files) = rewrite_data_urls("<p>kein bild</p>", "a");
        assert_eq!(new, "<p>kein bild</p>");
        assert!(files.is_empty());
    }
    #[test]
    fn referenced_paths_finds_sharded_refs() {
        let c = "<img src=\"noteimg://localhost/a/b/x.png\"><img src=\"noteimg://localhost/y.png\">";
        let r = referenced_paths(c);
        assert!(r.contains(&"a/b/x.png".to_string()));
        assert!(r.contains(&"y.png".to_string()));
    }
    #[test]
    fn gc_deletes_unreferenced_keeps_referenced() {
        let root = std::env::temp_dir().join(format!("notefix-gc-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(root.join("a/b")).unwrap();
        std::fs::write(root.join("a/b/x.png"), [1u8]).unwrap();
        std::fs::write(root.join("y.png"), [2u8]).unwrap();
        let mut refs = HashSet::new();
        refs.insert("a/b/x.png".to_string());
        gc_images(&root, &refs).unwrap();
        assert!(root.join("a/b/x.png").exists());
        assert!(!root.join("y.png").exists());
        std::fs::remove_dir_all(&root).ok();
    }
}
