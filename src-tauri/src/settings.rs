use rusqlite::Connection;

/// All settings as (key, value) pairs.
pub fn load_settings(conn: &Connection) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    rows.collect()
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (key, value),
    )?;
    Ok(())
}

/// Read a setting as an i64; missing/unparsable => default.
pub fn get_int(conn: &Connection, key: &str, default: i64) -> i64 {
    load_settings(conn)
        .ok()
        .and_then(|all| {
            all.into_iter()
                .find(|(k, _)| k == key)
                .and_then(|(_, v)| v.parse().ok())
        })
        .unwrap_or(default)
}

pub fn get_bool_default(conn: &Connection, key: &str, default: bool) -> bool {
    load_settings(conn)
        .ok()
        .and_then(|all| {
            all.into_iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v == "true")
        })
        .unwrap_or(default)
}

pub fn get_string(conn: &Connection, key: &str, default: &str) -> String {
    load_settings(conn)
        .ok()
        .and_then(|all| all.into_iter().find(|(k, _)| k == key).map(|(_, v)| v))
        .unwrap_or_else(|| default.to_string())
}

/// Read a setting as a bool ("true" => true, anything else / missing => false).
pub fn get_bool(conn: &Connection, key: &str) -> bool {
    load_settings(conn)
        .ok()
        .and_then(|all| {
            all.into_iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v == "true")
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate, storage::Store};

    fn conn() -> Store {
        let s = Store::open_in_memory().unwrap();
        migrate::run_migrations(&s.conn).unwrap();
        s
    }

    #[test]
    fn load_is_empty_initially() {
        let s = conn();
        assert!(load_settings(&s.conn).unwrap().is_empty());
    }

    #[test]
    fn set_then_load_returns_value() {
        let s = conn();
        set_setting(&s.conn, "pinnedDisplayMode", "sections").unwrap();
        let all = load_settings(&s.conn).unwrap();
        assert_eq!(
            all,
            vec![("pinnedDisplayMode".to_string(), "sections".to_string())]
        );
    }

    #[test]
    fn set_overwrites_existing() {
        let s = conn();
        set_setting(&s.conn, "k", "a").unwrap();
        set_setting(&s.conn, "k", "b").unwrap();
        let all = load_settings(&s.conn).unwrap();
        assert_eq!(all, vec![("k".to_string(), "b".to_string())]);
    }

    #[test]
    fn get_bool_reads_true_false_and_missing() {
        let s = conn();
        assert!(!get_bool(&s.conn, "startMinimized")); // missing => false
        set_setting(&s.conn, "startMinimized", "true").unwrap();
        assert!(get_bool(&s.conn, "startMinimized"));
        set_setting(&s.conn, "startMinimized", "false").unwrap();
        assert!(!get_bool(&s.conn, "startMinimized"));
    }

    #[test]
    fn get_string_default_and_set() {
        let s = conn();
        assert_eq!(get_string(&s.conn, "closeAction", "ask"), "ask");
        set_setting(&s.conn, "closeAction", "quit").unwrap();
        assert_eq!(get_string(&s.conn, "closeAction", "ask"), "quit");
    }
}
