//! Export / import of a local context as one zip bundle.
//!
//! A bundle holds `manifest.json`, a consistent snapshot of the context's
//! database (`notefix.db`, taken with `VACUUM INTO` — the app may be writing
//! to the live file) and the context's `images/` tree. Protected notes travel
//! as the ciphertext they already are in the database, with the vault record
//! next to them; no key material is ever written here and nothing in this
//! module logs note content.

use std::fmt::Display;
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Seek, Write};
use std::path::{Component, Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::storage::Store;

pub const FORMAT: &str = "notefix-context";
pub const FORMAT_VERSION: u32 = 1;

const MANIFEST_ENTRY: &str = "manifest.json";
const DB_ENTRY: &str = "notefix.db";
const IMAGES_DIR: &str = "images";

// The frontend maps these exact strings to its own messages
// (`ContextsPage` in `Settings.tsx`) — change them together.
const ERR_NOT_A_BUNDLE: &str = "bundle: not a Notefix context export";
const ERR_TOO_NEW: &str = "bundle: made by a newer Notefix — update this app first";
const ERR_UNSAFE_PATH: &str = "bundle: unsafe path in archive";
const ERR_NO_DB: &str = "bundle: the archive has no database";

/// A manifest is a few hundred bytes; never read an unbounded one.
const MANIFEST_MAX_BYTES: u64 = 64 * 1024;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub format: String,
    pub format_version: u32,
    pub label: String,
    pub app_version: String,
    pub schema_version: i64,
    /// RFC 3339, UTC.
    pub exported_at: String,
}

/// Just enough of a manifest to tell a foreign file or a newer bundle format
/// apart before committing to the full shape.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestHeader {
    #[serde(default)]
    format: String,
    #[serde(default)]
    format_version: u32,
}

fn msg(e: impl Display) -> String {
    e.to_string()
}

fn not_a_bundle<E>(_: E) -> String {
    ERR_NOT_A_BUNDLE.to_string()
}

fn invalid_db<E>(_: E) -> String {
    "bundle: the database in the archive is not valid".to_string()
}

/// `<out><suffix>`, next to `out`.
fn sibling(out: &Path, suffix: &str) -> Result<PathBuf, String> {
    let mut name = out
        .file_name()
        .ok_or_else(|| "bundle: the target is not a file path".to_string())?
        .to_os_string();
    name.push(suffix);
    Ok(out.with_file_name(name))
}

// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

/// Snapshot `db` (`VACUUM INTO` a temp file), write manifest + `notefix.db`
/// (deflate) + `images/**` (stored) into `out`. Writes to `<out>.partial`
/// first and renames on success; on any error the partial file is removed.
pub fn export(db: &Path, label: &str, app_version: &str, out: &Path) -> Result<(), String> {
    if !db.is_file() {
        return Err("bundle: the context has no database file".into());
    }
    let partial = sibling(out, ".partial")?;
    // Next to `out`: the snapshot holds the same plaintext the finished
    // archive does, so it is exactly as exposed as the user's chosen target.
    let snapshot = sibling(out, &format!(".{}.snapshot", uuid::Uuid::new_v4()))?;

    let result = write_bundle(db, label, app_version, &snapshot, &partial)
        .and_then(|()| fs::rename(&partial, out).map_err(msg));
    let _ = fs::remove_file(&snapshot);
    if result.is_err() {
        let _ = fs::remove_file(&partial);
    }
    result
}

fn write_bundle(
    db: &Path,
    label: &str,
    app_version: &str,
    snapshot: &Path,
    partial: &Path,
) -> Result<(), String> {
    snapshot_db(db, snapshot)?;
    let manifest = Manifest {
        format: FORMAT.into(),
        format_version: FORMAT_VERSION,
        label: label.into(),
        app_version: app_version.into(),
        schema_version: snapshot_schema_version(snapshot)?,
        exported_at: now_rfc3339()?,
    };

    let mut zip = ZipWriter::new(BufWriter::new(File::create(partial).map_err(msg)?));
    zip.start_file(
        MANIFEST_ENTRY,
        entry_options(CompressionMethod::Deflated, 0),
    )
    .map_err(msg)?;
    zip.write_all(&serde_json::to_vec_pretty(&manifest).map_err(msg)?)
        .map_err(msg)?;
    add_file(&mut zip, DB_ENTRY, snapshot, CompressionMethod::Deflated)?;
    // Images are already compressed formats: store them as they are.
    let images = crate::images::images_root_for(db);
    if images.is_dir() {
        add_dir(&mut zip, &images, IMAGES_DIR)?;
    }
    let file = zip.finish().map_err(msg)?.into_inner().map_err(msg)?;
    file.sync_all().map_err(msg)
}

fn now_rfc3339() -> Result<String, String> {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(msg)
}

/// A consistent copy of the live database. `VACUUM INTO` runs inside one read
/// transaction, so a concurrent writer neither corrupts nor tears it (copying
/// the file could do both). The source is opened read-only.
fn snapshot_db(db: &Path, snapshot: &Path) -> Result<(), String> {
    let target = snapshot
        .to_str()
        .ok_or_else(|| "bundle: the target path is not valid UTF-8".to_string())?;
    let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(msg)?;
    conn.execute("VACUUM INTO ?1", [target]).map_err(msg)?;
    Ok(())
}

fn snapshot_schema_version(snapshot: &Path) -> Result<i64, String> {
    let conn =
        Connection::open_with_flags(snapshot, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(msg)?;
    crate::migrate::get_meta(&conn, "schema_version")
        .map_err(msg)?
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| "bundle: the database has no schema version".to_string())
}

fn entry_options(method: CompressionMethod, len: u64) -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(method)
        .unix_permissions(0o644)
        // Only entries past 4 GiB need the zip64 form; keep the rest plain so
        // every unzip tool opens them.
        .large_file(len >= zip::ZIP64_BYTES_THR)
}

fn add_file<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    name: &str,
    path: &Path,
    method: CompressionMethod,
) -> Result<(), String> {
    let mut src = File::open(path).map_err(msg)?;
    let len = src.metadata().map_err(msg)?.len();
    zip.start_file(name, entry_options(method, len))
        .map_err(msg)?;
    io::copy(&mut src, zip).map_err(msg)?;
    Ok(())
}

/// Add every regular file under `dir` as `<prefix>/<relative path>`, in a
/// stable order. Symlinks and other special files are skipped.
fn add_dir<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    dir: &Path,
    prefix: &str,
) -> Result<(), String> {
    let mut children: Vec<_> = fs::read_dir(dir)
        .map_err(msg)?
        .collect::<Result<_, _>>()
        .map_err(msg)?;
    children.sort_by_key(|c| c.file_name());
    for child in children {
        let file_name = child.file_name();
        let name = file_name
            .to_str()
            .ok_or_else(|| "bundle: an image path is not valid UTF-8".to_string())?;
        let entry_name = format!("{prefix}/{name}");
        let kind = child.file_type().map_err(msg)?;
        if kind.is_dir() {
            add_dir(zip, &child.path(), &entry_name)?;
        } else if kind.is_file() {
            add_file(zip, &entry_name, &child.path(), CompressionMethod::Stored)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Import
// ---------------------------------------------------------------------------

/// Validate and extract `zip` into `dest` (which must not exist yet), then
/// open the DB and run migrations. On ANY error `dest` is removed entirely.
/// Returns the manifest as written by the exporting app.
///
/// `max_schema` is the newest database schema this app understands; a bundle
/// beyond it (or beyond [`FORMAT_VERSION`]) is refused.
pub fn import(zip: &Path, dest: &Path, max_schema: i64) -> Result<Manifest, String> {
    if dest.exists() {
        // Not ours to clean up.
        return Err("bundle: the destination already exists".into());
    }
    let mut archive = ZipArchive::new(File::open(zip).map_err(msg)?).map_err(not_a_bundle)?;
    // Everything refusable from the manifest alone is decided before the
    // filesystem is touched.
    let manifest = read_manifest(&mut archive, max_schema)?;

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(msg)?;
    }
    fs::create_dir(dest).map_err(msg)?;
    if let Err(e) = extract_and_open(&mut archive, dest, max_schema) {
        let _ = fs::remove_dir_all(dest);
        return Err(e);
    }
    Ok(manifest)
}

fn read_manifest(archive: &mut ZipArchive<File>, max_schema: i64) -> Result<Manifest, String> {
    let mut raw = Vec::new();
    archive
        .by_name(MANIFEST_ENTRY)
        .map_err(not_a_bundle)?
        .take(MANIFEST_MAX_BYTES)
        .read_to_end(&mut raw)
        .map_err(not_a_bundle)?;

    let header: ManifestHeader = serde_json::from_slice(&raw).map_err(not_a_bundle)?;
    if header.format != FORMAT {
        return Err(ERR_NOT_A_BUNDLE.into());
    }
    if header.format_version > FORMAT_VERSION {
        return Err(ERR_TOO_NEW.into());
    }
    let manifest: Manifest = serde_json::from_slice(&raw).map_err(not_a_bundle)?;
    if manifest.schema_version > max_schema {
        return Err(ERR_TOO_NEW.into());
    }
    Ok(manifest)
}

fn extract_and_open(
    archive: &mut ZipArchive<File>,
    dest: &Path,
    max_schema: i64,
) -> Result<(), String> {
    extract(archive, dest)?;
    let db = dest.join(DB_ENTRY);
    if !db.is_file() {
        return Err(ERR_NO_DB.into());
    }
    open_and_migrate(&db, max_schema)
}

/// Extract `notefix.db` and everything under `images/`; ignore the rest.
/// Every entry name is checked first, ignored ones included: an archive that
/// tries to climb out of `dest` is refused as a whole.
fn extract(archive: &mut ZipArchive<File>, dest: &Path) -> Result<(), String> {
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("bundle: could not read the archive: {e}"))?;
        let rel = entry
            .enclosed_name()
            .filter(|p| p.components().all(|c| matches!(c, Component::Normal(_))))
            .ok_or_else(|| ERR_UNSAFE_PATH.to_string())?;

        let wanted = rel == Path::new(DB_ENTRY)
            || (rel.starts_with(IMAGES_DIR) && rel != Path::new(IMAGES_DIR));
        if !wanted || entry.is_symlink() {
            continue;
        }
        let target = dest.join(&rel);
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(msg)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(msg)?;
        }
        let mut out = File::create(&target).map_err(msg)?;
        io::copy(&mut entry, &mut out)
            .map_err(|e| format!("bundle: could not read the archive: {e}"))?;
    }
    Ok(())
}

/// The extracted file must be a Notefix database of a schema this app knows;
/// older ones are migrated in place. The database's own `schema_version` is
/// what counts — the manifest is only a courtesy copy.
fn open_and_migrate(db: &Path, max_schema: i64) -> Result<(), String> {
    let store = Store::open(db).map_err(invalid_db)?;
    let has_notes = store
        .conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'notes'",
            [],
            |_| Ok(()),
        )
        .optional()
        .map_err(invalid_db)?
        .is_some();
    let version: i64 = crate::migrate::get_meta(&store.conn, "schema_version")
        .map_err(invalid_db)?
        .and_then(|v| v.parse().ok())
        .filter(|_| has_notes)
        .ok_or_else(|| invalid_db(()))?;
    if version > max_schema {
        return Err(ERR_TOO_NEW.into());
    }
    crate::migrate::run_migrations(&store.conn).map_err(msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrate::{run_migrations, SCHEMA_VERSION};
    use crate::storage::{Note, Store};
    use std::fs::{self, File};
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipArchive, ZipWriter};

    const NEWER: &str = "bundle: made by a newer Notefix — update this app first";
    const NOT_A_BUNDLE: &str = "bundle: not a Notefix context export";
    const UNSAFE: &str = "bundle: unsafe path in archive";
    const NO_DB: &str = "bundle: the archive has no database";
    const INVALID_DB: &str = "bundle: the database in the archive is not valid";

    fn migrated(path: &Path) -> Store {
        let s = Store::open(path).unwrap();
        run_migrations(&s.conn).unwrap();
        s
    }

    fn note(id: &str, content: &str, folder: Option<&str>) -> Note {
        Note {
            id: id.into(),
            content: content.into(),
            updated_at: 1_000,
            folder_id: folder.map(String::from),
            ..Default::default()
        }
    }

    fn put_image(db_dir: &Path, rel: &str, bytes: &[u8]) {
        let p = db_dir.join("images").join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, bytes).unwrap();
    }

    fn manifest_bytes(schema_version: i64) -> Vec<u8> {
        serde_json::to_vec(&Manifest {
            format: FORMAT.into(),
            format_version: FORMAT_VERSION,
            label: "Imported".into(),
            app_version: "1.2.3".into(),
            schema_version,
            exported_at: "2026-01-01T00:00:00Z".into(),
        })
        .unwrap()
    }

    /// Write a zip with exactly the given `(entry name, bytes)` pairs.
    fn build_zip(path: &Path, entries: &[(&str, Vec<u8>)]) {
        let mut z = ZipWriter::new(File::create(path).unwrap());
        for (name, data) in entries {
            z.start_file(*name, SimpleFileOptions::default()).unwrap();
            z.write_all(data).unwrap();
        }
        z.finish().unwrap();
    }

    /// The bytes of a freshly migrated, empty database.
    fn empty_db_bytes(dir: &Path) -> Vec<u8> {
        let p = dir.join("empty.db");
        drop(migrated(&p));
        fs::read(p).unwrap()
    }

    fn entries_of(zip: &Path) -> Vec<(String, CompressionMethod)> {
        let mut a = ZipArchive::new(File::open(zip).unwrap()).unwrap();
        (0..a.len())
            .map(|i| {
                let f = a.by_index(i).unwrap();
                (f.name().unwrap().to_string(), f.compression())
            })
            .collect()
    }

    fn leftovers(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn round_trip_carries_notes_images_vault_and_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        fs::create_dir_all(&src).unwrap();
        let db = src.join("notefix.db");
        let store = migrated(&db);
        crate::folders::create_folder(&store.conn, "f1", "Folder", None).unwrap();
        store
            .save_note(&note("n1", "<p>one</p>", Some("f1")))
            .unwrap();
        store.save_note(&note("n2", "<p>two</p>", None)).unwrap();
        store.save_note(&note("n3", "<p>secret</p>", None)).unwrap();
        let (_groups, dek) = crate::ops::vault_setup(&store, "pw").unwrap();
        crate::ops::encrypt_note_in_place(&store, "n3", &dek, 1).unwrap();
        put_image(
            &src,
            "aaaa/bbbb/cccc/dddd/eeeeeeeeeeee/pic.png",
            &[1, 2, 3, 4],
        );
        put_image(&src, "ffff/gggg/other.jpg", &[9, 8, 7]);
        let out = tmp.path().join("work.notefix.zip");

        export(&db, "Work", "9.9.9", &out).unwrap();

        assert!(out.is_file());
        assert_eq!(
            leftovers(tmp.path()),
            vec!["src", "work.notefix.zip"],
            "no .partial or snapshot file stays behind"
        );
        let dest = tmp.path().join("dest");
        let manifest = import(&out, &dest, SCHEMA_VERSION).unwrap();

        assert_eq!(manifest.format, FORMAT);
        assert_eq!(manifest.format_version, FORMAT_VERSION);
        assert_eq!(manifest.label, "Work");
        assert_eq!(manifest.app_version, "9.9.9");
        assert_eq!(manifest.schema_version, 15);
        assert!(
            time::OffsetDateTime::parse(
                &manifest.exported_at,
                &time::format_description::well_known::Rfc3339
            )
            .is_ok(),
            "exportedAt must be RFC 3339: {}",
            manifest.exported_at
        );

        let copy = Store::open(&dest.join("notefix.db")).unwrap();
        assert_eq!(copy.load_notes().unwrap(), store.load_notes().unwrap());
        let n3 = copy
            .load_notes()
            .unwrap()
            .into_iter()
            .find(|n| n.id == "n3")
            .unwrap();
        assert!(n3.protected, "the protected note stays ciphertext");
        assert!(!n3.content.contains("secret"));
        let folder_name: String = copy
            .conn
            .query_row("SELECT name FROM folders WHERE id = 'f1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(folder_name, "Folder");
        assert_eq!(copy.vault_record().unwrap(), store.vault_record().unwrap());
        // The same passphrase opens the imported note.
        let record = crate::ops::load_vault_record(&copy).unwrap();
        let unlocked = crate::ops::vault_unlock_passphrase(&record, "pw").unwrap();
        assert_eq!(
            crate::ops::open_content(&unlocked, "n3", &n3.content).unwrap(),
            "<p>secret</p>"
        );
        assert_eq!(
            fs::read(dest.join("images/aaaa/bbbb/cccc/dddd/eeeeeeeeeeee/pic.png")).unwrap(),
            vec![1, 2, 3, 4]
        );
        assert_eq!(
            fs::read(dest.join("images/ffff/gggg/other.jpg")).unwrap(),
            vec![9, 8, 7]
        );
    }

    #[test]
    fn the_archive_stores_images_and_deflates_the_database() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("notefix.db");
        drop(migrated(&db));
        put_image(tmp.path(), "a/b/pic.png", &[1; 64]);
        let out = tmp.path().join("x.zip");

        export(&db, "", "1.0.0", &out).unwrap();

        let entries = entries_of(&out);
        let method = |name: &str| entries.iter().find(|(n, _)| n == name).unwrap().1;
        assert_eq!(method("manifest.json"), CompressionMethod::Deflated);
        assert_eq!(method("notefix.db"), CompressionMethod::Deflated);
        assert_eq!(method("images/a/b/pic.png"), CompressionMethod::Stored);
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn an_empty_label_is_kept_as_is_in_the_manifest() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("notefix.db");
        drop(migrated(&db));
        let out = tmp.path().join("x.zip");

        export(&db, "", "1.0.0", &out).unwrap();
        let manifest = import(&out, &tmp.path().join("dest"), SCHEMA_VERSION).unwrap();

        assert_eq!(manifest.label, "");
    }

    #[test]
    fn export_snapshots_while_another_connection_is_open_with_a_pending_write() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("notefix.db");
        let live = migrated(&db);
        live.save_note(&note("committed", "<p>kept</p>", None))
            .unwrap();
        let other = Store::open(&db).unwrap();
        // An uncommitted write on the live connection: copying the file would
        // be unsafe, a snapshot must simply see the committed state.
        live.conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        live.save_note(&note("pending", "<p>not yet</p>", None))
            .unwrap();
        let out = tmp.path().join("x.zip");

        export(&db, "Live", "1.0.0", &out).unwrap();

        live.conn.execute_batch("COMMIT").unwrap();
        drop(other);
        let dest = tmp.path().join("dest");
        import(&out, &dest, SCHEMA_VERSION).unwrap();
        let ids: Vec<String> = Store::open(&dest.join("notefix.db"))
            .unwrap()
            .load_notes()
            .unwrap()
            .into_iter()
            .map(|n| n.id)
            .collect();
        assert_eq!(ids, vec!["committed"]);
    }

    #[test]
    fn export_of_a_missing_database_fails_and_creates_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().join("x.zip");

        assert!(export(&tmp.path().join("nope.db"), "", "1.0.0", &out).is_err());

        assert!(leftovers(tmp.path()).is_empty());
    }

    #[test]
    fn export_to_a_missing_directory_fails_without_leftovers() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("notefix.db");
        drop(migrated(&db));
        let out = tmp.path().join("no-such-dir").join("x.zip");

        assert!(export(&db, "", "1.0.0", &out).is_err());

        assert_eq!(leftovers(tmp.path()), vec!["notefix.db"]);
    }

    #[test]
    fn export_onto_a_directory_fails_and_removes_the_partial_file() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("notefix.db");
        drop(migrated(&db));
        let out = tmp.path().join("target.zip");
        fs::create_dir(&out).unwrap();

        assert!(export(&db, "", "1.0.0", &out).is_err());

        assert_eq!(leftovers(tmp.path()), vec!["notefix.db", "target.zip"]);
        assert!(out.is_dir(), "the existing directory is untouched");
    }

    #[test]
    fn export_replaces_an_existing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("notefix.db");
        drop(migrated(&db));
        let out = tmp.path().join("x.zip");
        fs::write(&out, b"old").unwrap();

        export(&db, "", "1.0.0", &out).unwrap();

        assert!(import(&out, &tmp.path().join("dest"), SCHEMA_VERSION).is_ok());
    }

    #[test]
    fn a_newer_schema_is_refused_and_leaves_nothing_behind() {
        let tmp = tempfile::tempdir().unwrap();
        let zip = tmp.path().join("b.zip");
        build_zip(
            &zip,
            &[
                ("manifest.json", manifest_bytes(SCHEMA_VERSION + 1)),
                ("notefix.db", empty_db_bytes(tmp.path())),
            ],
        );
        let dest = tmp.path().join("dest");

        assert_eq!(import(&zip, &dest, SCHEMA_VERSION).unwrap_err(), NEWER);

        assert!(!dest.exists());
    }

    #[test]
    fn a_newer_format_version_is_refused_even_if_its_manifest_shape_changed() {
        let tmp = tempfile::tempdir().unwrap();
        let zip = tmp.path().join("b.zip");
        let manifest = format!(r#"{{"format":"{FORMAT}","formatVersion":2,"somethingNew":true}}"#);
        build_zip(&zip, &[("manifest.json", manifest.into_bytes())]);
        let dest = tmp.path().join("dest");

        assert_eq!(import(&zip, &dest, SCHEMA_VERSION).unwrap_err(), NEWER);

        assert!(!dest.exists());
    }

    #[test]
    fn a_database_newer_than_its_manifest_claims_is_refused_and_removed() {
        let tmp = tempfile::tempdir().unwrap();
        let newer_db = tmp.path().join("newer.db");
        let s = migrated(&newer_db);
        crate::migrate::set_meta(&s.conn, "schema_version", "16").unwrap();
        drop(s);
        let zip = tmp.path().join("b.zip");
        build_zip(
            &zip,
            &[
                ("manifest.json", manifest_bytes(SCHEMA_VERSION)),
                ("notefix.db", fs::read(&newer_db).unwrap()),
            ],
        );
        let dest = tmp.path().join("dest");

        assert_eq!(import(&zip, &dest, SCHEMA_VERSION).unwrap_err(), NEWER);

        assert!(!dest.exists());
    }

    #[test]
    fn an_older_bundle_is_migrated_on_import() {
        let tmp = tempfile::tempdir().unwrap();
        let old_db = tmp.path().join("old.db");
        {
            let conn = rusqlite::Connection::open(&old_db).unwrap();
            conn.execute_batch(
                "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 CREATE TABLE notes (id TEXT PRIMARY KEY, content TEXT NOT NULL, updated_at INTEGER NOT NULL);
                 INSERT INTO notes VALUES ('old', '<p>old</p>', 5);",
            )
            .unwrap();
            crate::migrate::set_meta(&conn, "schema_version", "1").unwrap();
        }
        let zip = tmp.path().join("b.zip");
        build_zip(
            &zip,
            &[
                ("manifest.json", manifest_bytes(1)),
                ("notefix.db", fs::read(&old_db).unwrap()),
            ],
        );
        let dest = tmp.path().join("dest");

        let manifest = import(&zip, &dest, SCHEMA_VERSION).unwrap();

        assert_eq!(
            manifest.schema_version, 1,
            "the manifest is returned as written"
        );
        let copy = Store::open(&dest.join("notefix.db")).unwrap();
        assert_eq!(
            crate::migrate::get_meta(&copy.conn, "schema_version")
                .unwrap()
                .as_deref(),
            Some("15")
        );
        assert_eq!(copy.load_notes().unwrap()[0].id, "old");
    }

    #[test]
    fn a_zip_without_a_manifest_is_not_a_bundle() {
        let tmp = tempfile::tempdir().unwrap();
        let zip = tmp.path().join("b.zip");
        build_zip(&zip, &[("notefix.db", empty_db_bytes(tmp.path()))]);
        let dest = tmp.path().join("dest");

        assert_eq!(
            import(&zip, &dest, SCHEMA_VERSION).unwrap_err(),
            NOT_A_BUNDLE
        );

        assert!(!dest.exists());
    }

    #[test]
    fn a_foreign_format_is_not_a_bundle() {
        let tmp = tempfile::tempdir().unwrap();
        let zip = tmp.path().join("b.zip");
        let manifest = br#"{"format":"something-else","formatVersion":1}"#.to_vec();
        build_zip(&zip, &[("manifest.json", manifest)]);
        let dest = tmp.path().join("dest");

        assert_eq!(
            import(&zip, &dest, SCHEMA_VERSION).unwrap_err(),
            NOT_A_BUNDLE
        );

        assert!(!dest.exists());
    }

    #[test]
    fn an_unparsable_manifest_is_not_a_bundle() {
        let tmp = tempfile::tempdir().unwrap();
        let zip = tmp.path().join("b.zip");
        build_zip(&zip, &[("manifest.json", b"{nope".to_vec())]);
        let dest = tmp.path().join("dest");

        assert_eq!(
            import(&zip, &dest, SCHEMA_VERSION).unwrap_err(),
            NOT_A_BUNDLE
        );

        assert!(!dest.exists());
    }

    #[test]
    fn a_file_that_is_not_a_zip_is_not_a_bundle() {
        let tmp = tempfile::tempdir().unwrap();
        let zip = tmp.path().join("b.zip");
        fs::write(&zip, b"%PDF-1.7 definitely not a zip").unwrap();
        let dest = tmp.path().join("dest");

        assert_eq!(
            import(&zip, &dest, SCHEMA_VERSION).unwrap_err(),
            NOT_A_BUNDLE
        );

        assert!(!dest.exists());
    }

    #[test]
    fn a_missing_file_is_reported_and_creates_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("dest");

        assert!(import(&tmp.path().join("gone.zip"), &dest, SCHEMA_VERSION).is_err());

        assert!(!dest.exists());
    }

    #[test]
    fn a_zip_without_a_database_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let zip = tmp.path().join("b.zip");
        build_zip(&zip, &[("manifest.json", manifest_bytes(SCHEMA_VERSION))]);
        let dest = tmp.path().join("dest");

        assert_eq!(import(&zip, &dest, SCHEMA_VERSION).unwrap_err(), NO_DB);

        assert!(!dest.exists());
    }

    #[test]
    fn a_non_database_file_is_refused_and_removed() {
        let tmp = tempfile::tempdir().unwrap();
        let zip = tmp.path().join("b.zip");
        build_zip(
            &zip,
            &[
                ("manifest.json", manifest_bytes(SCHEMA_VERSION)),
                ("notefix.db", b"this is not a sqlite file at all".to_vec()),
            ],
        );
        let dest = tmp.path().join("dest");

        assert_eq!(import(&zip, &dest, SCHEMA_VERSION).unwrap_err(), INVALID_DB);

        assert!(!dest.exists());
    }

    #[test]
    fn a_database_without_a_notes_table_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let foreign = tmp.path().join("foreign.db");
        {
            let conn = rusqlite::Connection::open(&foreign).unwrap();
            conn.execute_batch(
                "CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO meta VALUES ('schema_version', '15');",
            )
            .unwrap();
        }
        let zip = tmp.path().join("b.zip");
        build_zip(
            &zip,
            &[
                ("manifest.json", manifest_bytes(SCHEMA_VERSION)),
                ("notefix.db", fs::read(&foreign).unwrap()),
            ],
        );
        let dest = tmp.path().join("dest");

        assert_eq!(import(&zip, &dest, SCHEMA_VERSION).unwrap_err(), INVALID_DB);

        assert!(!dest.exists());
    }

    #[test]
    fn path_traversal_entries_are_refused_and_write_nothing_outside() {
        for evil in ["../evil", "images/../../evil", "images/a/../../../evil"] {
            let tmp = tempfile::tempdir().unwrap();
            let work = tmp.path().join("work");
            fs::create_dir_all(&work).unwrap();
            let zip = work.join("b.zip");
            build_zip(
                &zip,
                &[
                    ("manifest.json", manifest_bytes(SCHEMA_VERSION)),
                    ("notefix.db", empty_db_bytes(tmp.path())),
                    (evil, b"pwned".to_vec()),
                ],
            );
            let dest = work.join("contexts").join("new");

            assert_eq!(
                import(&zip, &dest, SCHEMA_VERSION).unwrap_err(),
                UNSAFE,
                "{evil}"
            );

            assert!(!dest.exists(), "{evil}");
            assert!(!work.join("evil").exists(), "{evil}");
            assert!(!tmp.path().join("evil").exists(), "{evil}");
        }
    }

    #[test]
    fn only_the_database_and_images_are_extracted() {
        let tmp = tempfile::tempdir().unwrap();
        let zip = tmp.path().join("b.zip");
        build_zip(
            &zip,
            &[
                ("manifest.json", manifest_bytes(SCHEMA_VERSION)),
                ("notefix.db", empty_db_bytes(tmp.path())),
                ("images/a/b/pic.png", vec![5, 5, 5]),
                ("README.txt", b"hello".to_vec()),
                ("__MACOSX/._notefix.db", b"junk".to_vec()),
                ("settings.json", b"{}".to_vec()),
            ],
        );
        let dest = tmp.path().join("dest");

        import(&zip, &dest, SCHEMA_VERSION).unwrap();

        assert_eq!(leftovers(&dest), vec!["images", "notefix.db"]);
        assert_eq!(
            fs::read(dest.join("images/a/b/pic.png")).unwrap(),
            vec![5, 5, 5]
        );
    }

    #[test]
    fn an_existing_destination_is_refused_and_left_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let zip = tmp.path().join("b.zip");
        build_zip(
            &zip,
            &[
                ("manifest.json", manifest_bytes(SCHEMA_VERSION)),
                ("notefix.db", empty_db_bytes(tmp.path())),
            ],
        );
        let dest = tmp.path().join("dest");
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("keep.txt"), b"mine").unwrap();

        assert!(import(&zip, &dest, SCHEMA_VERSION).is_err());

        assert_eq!(fs::read(dest.join("keep.txt")).unwrap(), b"mine");
    }

    #[test]
    fn corrupt_entry_data_is_refused_without_leftovers() {
        let tmp = tempfile::tempdir().unwrap();
        let zip = tmp.path().join("b.zip");
        let mut z = ZipWriter::new(File::create(&zip).unwrap());
        let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        z.start_file("manifest.json", stored).unwrap();
        z.write_all(&manifest_bytes(SCHEMA_VERSION)).unwrap();
        z.start_file("notefix.db", stored).unwrap();
        z.write_all(&empty_db_bytes(tmp.path())).unwrap();
        z.finish().unwrap();
        // Stored data sits verbatim in the file: flip a byte inside the
        // database so its checksum no longer matches.
        let mut bytes = fs::read(&zip).unwrap();
        let magic = b"SQLite format 3";
        let at = bytes.windows(magic.len()).position(|w| w == magic).unwrap();
        bytes[at + 100] ^= 0xff;
        fs::write(&zip, bytes).unwrap();
        let dest = tmp.path().join("dest");

        let err = import(&zip, &dest, SCHEMA_VERSION).unwrap_err();

        assert!(
            err.starts_with("bundle: could not read the archive"),
            "{err}"
        );
        assert!(!dest.exists());
    }

    #[test]
    fn a_truncated_archive_is_refused_without_leftovers() {
        let tmp = tempfile::tempdir().unwrap();
        let db = tmp.path().join("notefix.db");
        drop(migrated(&db));
        let good = tmp.path().join("good.zip");
        export(&db, "", "1.0.0", &good).unwrap();
        let bytes = fs::read(&good).unwrap();
        let cut = tmp.path().join("cut.zip");
        fs::write(&cut, &bytes[..bytes.len() / 2]).unwrap();
        let dest = tmp.path().join("dest");

        assert!(import(&cut, &dest, SCHEMA_VERSION).is_err());

        assert!(!dest.exists());
    }
}
