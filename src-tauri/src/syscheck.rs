use std::path::Path;

/// True, wenn in `dir` eine Datei geschrieben (und wieder gelöscht) werden kann.
pub fn is_writable(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    let probe = dir.join(format!(".notefix-write-test-{}", uuid::Uuid::new_v4()));
    match std::fs::write(&probe, b"x") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn writable_for_temp_dir_not_for_missing() {
        let dir = std::env::temp_dir().join(format!("notefix-sc-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        assert!(is_writable(&dir));
        assert!(!is_writable(&dir.join("does-not-exist")));
        std::fs::remove_dir_all(&dir).ok();
    }
}
