use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub position: i64,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub sort: String,
    #[serde(default)]
    pub updated_at: i64,
    #[serde(default)]
    pub deleted_at: Option<i64>,
    #[serde(default)]
    pub dirty: bool,
}

pub enum DeleteMode {
    Reparent,
    Recursive,
}

impl DeleteMode {
    pub fn from_str(s: &str) -> Self {
        if s == "recursive" { DeleteMode::Recursive } else { DeleteMode::Reparent }
    }
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

pub fn load_folders(conn: &Connection) -> rusqlite::Result<Vec<Folder>> {
    let mut stmt = conn.prepare("SELECT id, name, parent_id, position, icon, color, sort, updated_at, deleted_at, dirty FROM folders WHERE deleted_at IS NULL ORDER BY position, name")?;
    let rows = stmt.query_map([], |r| Ok(Folder {
        id: r.get(0)?, name: r.get(1)?, parent_id: r.get(2)?, position: r.get(3)?, icon: r.get(4)?, color: r.get(5)?, sort: r.get(6)?,
        updated_at: r.get(7)?, deleted_at: r.get(8)?, dirty: r.get(9)?,
    }))?;
    rows.collect()
}

pub fn create_folder(conn: &Connection, id: &str, name: &str, parent_id: Option<&str>) -> rusqlite::Result<()> {
    let position: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position), 0) + 1 FROM folders WHERE parent_id IS ?1",
        [parent_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO folders (id, name, parent_id, position, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        (id, name, parent_id, position, now_ms()),
    )?;
    Ok(())
}

pub fn rename_folder(conn: &Connection, id: &str, name: &str) -> rusqlite::Result<()> {
    conn.execute("UPDATE folders SET name = ?2 WHERE id = ?1", (id, name))?;
    Ok(())
}

pub fn set_folder_icon(conn: &Connection, id: &str, icon: &str) -> rusqlite::Result<()> {
    conn.execute("UPDATE folders SET icon = ?2 WHERE id = ?1", (id, icon))?;
    Ok(())
}

pub fn set_folder_color(conn: &Connection, id: &str, color: &str) -> rusqlite::Result<()> {
    conn.execute("UPDATE folders SET color = ?2 WHERE id = ?1", (id, color))?;
    Ok(())
}

pub fn set_folder_sort(conn: &Connection, id: &str, sort: &str) -> rusqlite::Result<()> {
    conn.execute("UPDATE folders SET sort = ?2 WHERE id = ?1", (id, sort))?;
    Ok(())
}

/// All folder ids strictly below `id`.
pub fn descendants(conn: &Connection, id: &str) -> rusqlite::Result<Vec<String>> {
    let mut out = Vec::new();
    let mut queue = vec![id.to_string()];
    while let Some(cur) = queue.pop() {
        let mut stmt = conn.prepare("SELECT id FROM folders WHERE parent_id = ?1")?;
        let kids: Vec<String> = stmt.query_map([&cur], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
        for k in kids {
            out.push(k.clone());
            queue.push(k);
        }
    }
    Ok(out)
}

/// Re-parent a folder. Rejects moving a folder into itself or a descendant.
pub fn move_folder(conn: &Connection, id: &str, new_parent_id: Option<&str>) -> rusqlite::Result<()> {
    if let Some(p) = new_parent_id {
        if p == id || descendants(conn, id)?.iter().any(|d| d == p) {
            return Err(rusqlite::Error::InvalidQuery);
        }
    }
    conn.execute("UPDATE folders SET parent_id = ?2 WHERE id = ?1", (id, new_parent_id))?;
    Ok(())
}

pub fn delete_folder(conn: &Connection, id: &str, mode: DeleteMode) -> rusqlite::Result<()> {
    match mode {
        DeleteMode::Reparent => {
            let parent: Option<String> = conn
                .query_row("SELECT parent_id FROM folders WHERE id = ?1", [id], |r| r.get(0))
                .optional()?
                .flatten();
            conn.execute("UPDATE folders SET parent_id = ?2 WHERE parent_id = ?1", (id, parent.as_deref()))?;
            conn.execute("UPDATE notes SET folder_id = ?2 WHERE folder_id = ?1", (id, parent.as_deref()))?;
            conn.execute("DELETE FROM folders WHERE id = ?1", [id])?;
        }
        DeleteMode::Recursive => {
            let mut ids = descendants(conn, id)?;
            ids.push(id.to_string());
            for fid in &ids {
                conn.execute("DELETE FROM notes WHERE folder_id = ?1", [fid])?;
                conn.execute("DELETE FROM folders WHERE id = ?1", [fid])?;
            }
        }
    }
    Ok(())
}

/// Server-context folder delete: tombstone the folder (and, recursively, its
/// subtree's notes+folders) or reparent children, marking affected rows dirty.
pub fn sync_delete_folder(conn: &Connection, id: &str, mode: DeleteMode) -> rusqlite::Result<()> {
    let now = now_ms();
    match mode {
        DeleteMode::Reparent => {
            let parent: Option<String> = conn
                .query_row("SELECT parent_id FROM folders WHERE id = ?1", [id], |r| r.get(0))
                .optional()?
                .flatten();
            conn.execute("UPDATE folders SET parent_id = ?2, updated_at = ?3, dirty = 1 WHERE parent_id = ?1", (id, parent.as_deref(), now))?;
            conn.execute("UPDATE notes SET folder_id = ?2, updated_at = ?3, dirty = 1 WHERE folder_id = ?1", (id, parent.as_deref(), now))?;
            conn.execute("UPDATE folders SET deleted_at = ?2, updated_at = ?2, dirty = 1 WHERE id = ?1", (id, now))?;
        }
        DeleteMode::Recursive => {
            let mut ids = descendants(conn, id)?;
            ids.push(id.to_string());
            for fid in &ids {
                conn.execute("UPDATE notes SET deleted_at = ?2, updated_at = ?2, dirty = 1 WHERE folder_id = ?1 AND deleted_at IS NULL", (fid, now))?;
                conn.execute("UPDATE folders SET deleted_at = ?2, updated_at = ?2, dirty = 1 WHERE id = ?1", (fid, now))?;
            }
        }
    }
    Ok(())
}

/// Set parent + position for each id in order. Rejects moving a folder under its own descendant.
pub fn reorder_folders(conn: &Connection, parent_id: Option<&str>, ids: &[String]) -> rusqlite::Result<()> {
    if let Some(p) = parent_id {
        for id in ids {
            if p == id || descendants(conn, id)?.iter().any(|d| d == p) {
                return Err(rusqlite::Error::InvalidQuery);
            }
        }
    }
    for (i, id) in ids.iter().enumerate() {
        conn.execute("UPDATE folders SET parent_id = ?2, position = ?3 WHERE id = ?1", (id, parent_id, i as i64))?;
    }
    Ok(())
}

pub fn touch_folder(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    conn.execute("UPDATE folders SET updated_at = ?2, dirty = 1 WHERE id = ?1", (id, now_ms()))?;
    Ok(())
}

pub fn tombstone_folder(conn: &Connection, id: &str) -> rusqlite::Result<()> {
    let now = now_ms();
    conn.execute("UPDATE folders SET deleted_at = ?2, dirty = 1, updated_at = ?2 WHERE id = ?1", (id, now))?;
    Ok(())
}

pub fn load_dirty_folders(conn: &Connection) -> rusqlite::Result<Vec<Folder>> {
    let mut stmt = conn.prepare("SELECT id, name, parent_id, position, icon, color, sort, updated_at, deleted_at, dirty FROM folders WHERE dirty = 1")?;
    let rows = stmt.query_map([], |r| Ok(Folder {
        id: r.get(0)?, name: r.get(1)?, parent_id: r.get(2)?, position: r.get(3)?, icon: r.get(4)?, color: r.get(5)?, sort: r.get(6)?,
        updated_at: r.get(7)?, deleted_at: r.get(8)?, dirty: r.get(9)?,
    }))?;
    rows.collect()
}

pub fn clear_folder_dirty(conn: &Connection, ids: &[String]) -> rusqlite::Result<()> {
    for id in ids { conn.execute("UPDATE folders SET dirty = 0 WHERE id = ?1", [id])?; }
    Ok(())
}

pub fn upsert_folder_from_server(conn: &Connection, f: &Folder) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO folders (id, name, parent_id, position, icon, color, sort, created_at, updated_at, deleted_at, dirty)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, ?9, 0)
         ON CONFLICT(id) DO UPDATE SET
            name=excluded.name, parent_id=excluded.parent_id, position=excluded.position,
            icon=excluded.icon, color=excluded.color, sort=excluded.sort,
            updated_at=excluded.updated_at, deleted_at=excluded.deleted_at, dirty=0",
        (&f.id, &f.name, &f.parent_id, f.position, &f.icon, &f.color, &f.sort, f.updated_at, f.deleted_at),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{migrate, storage::{Note, Store}};

    fn store() -> Store {
        let s = Store::open_in_memory().unwrap();
        migrate::run_migrations(&s.conn).unwrap();
        s
    }

    fn note_in(id: &str, folder: &str) -> Note {
        Note { id: id.into(), content: "<p>x</p>".into(), updated_at: 1, pinned: false, archived: false, color: String::new(), due_at: None, folder_id: Some(folder.into()), position: 0, deleted_at: None, dirty: false }
    }

    #[test]
    fn create_and_load_orders_by_position() {
        let s = store();
        create_folder(&s.conn, "a", "A", None).unwrap();
        create_folder(&s.conn, "b", "B", None).unwrap();
        let ids: Vec<String> = load_folders(&s.conn).unwrap().into_iter().map(|f| f.id).collect();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn set_icon_and_color_persist_and_default_empty() {
        let s = store();
        create_folder(&s.conn, "a", "A", None).unwrap();
        let f = load_folders(&s.conn).unwrap();
        assert_eq!(f[0].icon, "");
        assert_eq!(f[0].color, "");
        set_folder_icon(&s.conn, "a", "fa:star").unwrap();
        set_folder_color(&s.conn, "a", "#22c55e").unwrap();
        let f = load_folders(&s.conn).unwrap();
        assert_eq!(f[0].icon, "fa:star");
        assert_eq!(f[0].color, "#22c55e");
    }

    #[test]
    fn rename_changes_name() {
        let s = store();
        create_folder(&s.conn, "a", "A", None).unwrap();
        rename_folder(&s.conn, "a", "Neu").unwrap();
        assert_eq!(load_folders(&s.conn).unwrap()[0].name, "Neu");
    }

    #[test]
    fn move_into_own_descendant_is_rejected() {
        let s = store();
        create_folder(&s.conn, "a", "A", None).unwrap();
        create_folder(&s.conn, "b", "B", Some("a")).unwrap();
        assert!(move_folder(&s.conn, "a", Some("b")).is_err());
        assert!(move_folder(&s.conn, "a", Some("a")).is_err());
    }

    #[test]
    fn delete_reparent_moves_children_and_notes_to_parent() {
        let s = store();
        create_folder(&s.conn, "p", "P", None).unwrap();
        create_folder(&s.conn, "c", "C", Some("p")).unwrap();
        create_folder(&s.conn, "g", "G", Some("c")).unwrap();
        s.save_note(&note_in("n", "c")).unwrap();
        delete_folder(&s.conn, "c", DeleteMode::Reparent).unwrap();
        // g now under p, n now under p, c gone
        let folders = load_folders(&s.conn).unwrap();
        assert!(folders.iter().all(|f| f.id != "c"));
        assert_eq!(folders.iter().find(|f| f.id == "g").unwrap().parent_id.as_deref(), Some("p"));
        assert_eq!(s.load_notes().unwrap()[0].folder_id.as_deref(), Some("p"));
    }

    #[test]
    fn reorder_folders_sets_parent_and_position_and_blocks_cycles() {
        let s = store();
        create_folder(&s.conn, "a", "A", None).unwrap();
        create_folder(&s.conn, "b", "B", None).unwrap();
        create_folder(&s.conn, "c", "C", Some("a")).unwrap();
        // move b under a, ordered [c, b]
        reorder_folders(&s.conn, Some("a"), &["c".to_string(), "b".to_string()]).unwrap();
        let f = load_folders(&s.conn).unwrap();
        let b = f.iter().find(|x| x.id == "b").unwrap();
        assert_eq!(b.parent_id.as_deref(), Some("a"));
        assert_eq!(b.position, 1);
        // cycle: move a under its descendant c
        assert!(reorder_folders(&s.conn, Some("c"), &["a".to_string()]).is_err());
    }

    #[test]
    fn set_sort_persists_and_defaults_manual() {
        let s = store();
        create_folder(&s.conn, "a", "A", None).unwrap();
        assert_eq!(load_folders(&s.conn).unwrap()[0].sort, "manual");
        set_folder_sort(&s.conn, "a", "titleAsc").unwrap();
        assert_eq!(load_folders(&s.conn).unwrap()[0].sort, "titleAsc");
    }

    #[test]
    fn delete_recursive_removes_subtree_and_its_notes() {
        let s = store();
        create_folder(&s.conn, "c", "C", None).unwrap();
        create_folder(&s.conn, "g", "G", Some("c")).unwrap();
        s.save_note(&note_in("n", "g")).unwrap();
        delete_folder(&s.conn, "c", DeleteMode::Recursive).unwrap();
        assert!(load_folders(&s.conn).unwrap().is_empty());
        assert!(s.load_notes().unwrap().is_empty());
    }

    #[test]
    fn touch_and_dirty_collect_for_folders() {
        let s = crate::storage::Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        create_folder(&s.conn, "f1", "Work", None).unwrap();
        touch_folder(&s.conn, "f1").unwrap();
        assert_eq!(load_dirty_folders(&s.conn).unwrap().len(), 1);
        clear_folder_dirty(&s.conn, &["f1".into()]).unwrap();
        assert!(load_dirty_folders(&s.conn).unwrap().is_empty());
    }
}
