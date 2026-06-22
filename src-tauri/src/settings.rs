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
        assert_eq!(all, vec![("pinnedDisplayMode".to_string(), "sections".to_string())]);
    }

    #[test]
    fn set_overwrites_existing() {
        let s = conn();
        set_setting(&s.conn, "k", "a").unwrap();
        set_setting(&s.conn, "k", "b").unwrap();
        let all = load_settings(&s.conn).unwrap();
        assert_eq!(all, vec![("k".to_string(), "b".to_string())]);
    }
}
