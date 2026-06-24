use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

/// Pure: extract `dbPath` from config JSON, else the default.
pub fn parse_db_path(json: &str, default: &Path) -> PathBuf {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v.get("dbPath").and_then(|p| p.as_str()).map(PathBuf::from))
        .unwrap_or_else(|| default.to_path_buf())
}

/// Pure: serialize a db path to config JSON.
pub fn serialize_db_path(path: &Path) -> String {
    serde_json::json!({ "dbPath": path.to_string_lossy() }).to_string()
}

fn app_data(app: &AppHandle) -> PathBuf {
    app.path().app_data_dir().expect("app data dir")
}

pub fn config_path(app: &AppHandle) -> PathBuf {
    app_data(app).join("config.json")
}

pub fn default_db_path(app: &AppHandle) -> PathBuf {
    app_data(app).join("notefix.db")
}

pub fn profiles_path(app: &AppHandle) -> PathBuf {
    app_data(app).join("profiles.json")
}

pub fn contexts_dir(app: &AppHandle) -> PathBuf {
    app_data(app).join("contexts")
}

pub fn read_db_path(app: &AppHandle) -> PathBuf {
    let default = default_db_path(app);
    match std::fs::read_to_string(config_path(app)) {
        Ok(json) => parse_db_path(&json, &default),
        Err(_) => default,
    }
}

pub fn write_db_path(app: &AppHandle, path: &Path) -> std::io::Result<()> {
    std::fs::write(config_path(app), serialize_db_path(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    #[test]
    fn parse_returns_path_or_default() {
        let d = Path::new("/def/notefix.db");
        assert_eq!(
            parse_db_path(r#"{"dbPath":"/custom/notefix.db"}"#, d),
            PathBuf::from("/custom/notefix.db")
        );
        assert_eq!(parse_db_path("garbage", d), d.to_path_buf());
        assert_eq!(parse_db_path("{}", d), d.to_path_buf());
    }

    #[test]
    fn serialize_round_trips() {
        let p = Path::new("/x/notefix.db");
        let json = serialize_db_path(p);
        assert!(json.contains("dbPath"));
        assert_eq!(parse_db_path(&json, Path::new("/def/notefix.db")), p.to_path_buf());
    }
}
