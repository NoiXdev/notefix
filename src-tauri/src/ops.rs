//! Pure, testable core of the Tauri command layer.
//!
//! Every `#[tauri::command]` in [`crate::commands`] that carries real logic —
//! validation, branching, multi-step store/vault mutations — delegates it to a
//! function here. The commands keep only what genuinely needs Tauri: acquiring
//! the managed `State` mutexes, emitting events, and touching windows/tray.
//!
//! The functions below therefore take plain arguments (`&Store`,
//! `&mut Registry`, `Option<&Dek>`, `&Path`, values) and return
//! `Result<T, String>`, which makes them unit-testable against
//! `Store::open_in_memory()` + `crate::migrate::run_migrations` without any
//! Tauri harness. This follows the pattern the vault work already established
//! with `reconcile_folder_move` / `reconcile_reorder` / `encrypt_note_in_place`.
//!
//! Security note: the vault paths here are the enforcement point for the
//! "`content` is ciphertext *iff* `notes.protected = 1`" invariant and for
//! every `"vault locked"` / `"note is protected by its folder"` refusal. A
//! locked vault is represented as `dek: None`; nothing in this module ever
//! logs key or plaintext material.

use std::path::{Path, PathBuf};

use base64::Engine;

use crate::profiles::{ContextEntry, Registry};
use crate::storage::{Note, SearchHit, Store};
use crate::vault::aead::Dek;
use crate::vault::VaultRecord;

// ---------------------------------------------------------------------------
// Serializable results returned across the Tauri boundary
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbLocationResult {
    pub mode: String,
    pub path: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextInfo {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub path: String,
    pub server_url: String,
    pub workspace_id: String,
    pub active: bool,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PathChecks {
    pub db_writable: bool,
    pub images_writable: bool,
    pub db_path: String,
    pub images_path: String,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub state: String, // "local" | "unbound" | "syncing" | "synced" | "offline"
    pub last_synced_at: i64,
    pub pending: i64,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub exists: bool,
    pub unlocked: bool,
    pub biometric: bool,
}

// ---------------------------------------------------------------------------
// Protected-note crypto primitives
//
// The physical-state invariant that makes the rest straightforward: `content`
// is ciphertext *iff* `notes.protected = 1`. `folders.locked` is a separate
// "intent" flag. Reads/writes always consult `protected` directly (cheap);
// only the transition ops walk the folder tree.
// ---------------------------------------------------------------------------

/// Seals a note's plaintext HTML into a base64-encoded AEAD blob for storage
/// in the `notes.content` column. The note id is bound in as associated data
/// so a sealed blob can't be silently reattached to a different note's row.
///
/// Deliberately private: this produces ciphertext WITHOUT touching the
/// `protected` flag, so calling it on its own would break the
/// "`content` is ciphertext iff `protected = 1`" invariant. Every caller goes
/// through a transition op ([`encrypt_note_in_place`] or [`save_note`]) that
/// sets the flag in the same breath.
fn seal_content(dek: &Dek, note_id: &str, html: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(crate::vault::aead::seal(
        dek,
        note_id.as_bytes(),
        html.as_bytes(),
    ))
}

/// Reverses [`seal_content`]: base64-decode, open the AEAD blob (checking the
/// note id as associated data), then validate the plaintext is UTF-8. Every
/// failure maps to a plain `String` — never key material or plaintext.
///
/// `pub(crate)` (and re-exported as `crate::commands::open_content`) so the MCP
/// surface can decrypt an effectively-protected note's content for a read tool
/// when `mcpProtectedAccess` allows it — see `mcp::StoreAccess::decrypt_protected`.
pub(crate) fn open_content(dek: &Dek, note_id: &str, stored: &str) -> Result<String, String> {
    let blob = base64::engine::general_purpose::STANDARD
        .decode(stored)
        .map_err(|e| e.to_string())?;
    let plaintext =
        crate::vault::aead::open(dek, note_id.as_bytes(), &blob).map_err(|e| e.to_string())?;
    String::from_utf8(plaintext).map_err(|e| e.to_string())
}

/// True if any ancestor folder of `note_id` (not counting the note's own
/// `protected` flag — see `Store::is_effectively_protected` for that) is
/// `locked`. Cycle-safe via a visited set, mirroring the walk in
/// `Store::is_effectively_protected`.
fn has_locked_ancestor_folder(store: &Store, note_id: &str) -> Result<bool, String> {
    use rusqlite::OptionalExtension;

    let folder_id: Option<String> = store
        .conn
        .query_row(
            "SELECT folder_id FROM notes WHERE id = ?1",
            [note_id],
            |r| r.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .flatten();
    folder_chain_has_lock(store, folder_id.as_deref())
}

/// True if `starting_folder_id` or any of its ancestors is `locked`.
/// Cycle-safe via a visited set. Shared by [`has_locked_ancestor_folder`]
/// (starts from a note's *current* `folder_id`) and [`reconcile_folder_move`] /
/// [`reconcile_reorder`] (start from a move's *destination* `folder_id`,
/// checked before the move is performed). Thin `String`-error wrapper over
/// `Store::folder_chain_has_lock`.
fn folder_chain_has_lock(store: &Store, starting_folder_id: Option<&str>) -> Result<bool, String> {
    store
        .folder_chain_has_lock(starting_folder_id)
        .map_err(|e| e.to_string())
}

/// Encrypt one currently-plaintext note in place under `dek`: seal its content
/// (binding the note id as AEAD associated data), flip `protected`, mark it
/// dirty so the ciphertext + `protected = 1` propagate on sync, and purge its
/// now-defunct plaintext revision history. The single encrypt-on-transition
/// primitive shared by every "plaintext note enters a locked context" path
/// ([`reconcile_folder_move`], [`reconcile_reorder`], [`set_note_protected`],
/// [`set_folder_locked`]), so all of them mark the row dirty identically.
///
/// `pub(crate)` (and re-exported as `crate::commands::encrypt_note_in_place`)
/// so the MCP surface can reuse it too: when `mcpProtectedAccess` is
/// "readwrite" and the vault is unlocked, a write tool writes its new plaintext
/// into `content` (still under the same store lock) and immediately calls this
/// to reseal it — see `mcp::StoreAccess::write_protected`.
pub(crate) fn encrypt_note_in_place(store: &Store, id: &str, dek: &Dek) -> Result<(), String> {
    let plaintext = store
        .load_note_content(id)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    // Capture the title from the PLAINTEXT before it's sealed — the title is
    // deliberately left as visible metadata even once the body is ciphertext
    // (see `Store::set_title` / `Note::title`), so it must never be derived
    // from the ciphertext.
    let title = crate::storage::note_preview(&plaintext);
    let sealed = seal_content(dek, id, &plaintext);
    store
        .set_content_silent(id, &sealed)
        .map_err(|e| e.to_string())?;
    store.set_title(id, &title).map_err(|e| e.to_string())?;
    store
        .set_note_protected(id, true)
        .map_err(|e| e.to_string())?;
    store
        .mark_note_dirty_if_syncing(id)
        .map_err(|e| e.to_string())?;
    crate::revisions::delete_revisions(&store.conn, id).map_err(|e| e.to_string())?;
    Ok(())
}

/// Decrypt one currently-encrypted note in place under `dek`: open its stored
/// ciphertext, write back the plaintext, and clear `protected`. The inverse of
/// [`encrypt_note_in_place`], shared by `note_set_protected(false)` and
/// `folder_set_locked(false)`.
///
/// Deliberately does NOT restore revision history (it was purged on the way in)
/// and does NOT re-mark the row dirty beyond what `set_content_silent` /
/// `set_note_protected` already do — matching the pre-refactor behavior of both
/// callers exactly.
fn decrypt_note_in_place(store: &Store, id: &str, dek: &Dek) -> Result<(), String> {
    let ciphertext = store
        .load_note_content(id)
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let plaintext = open_content(dek, id, &ciphertext)?;
    store
        .set_content_silent(id, &plaintext)
        .map_err(|e| e.to_string())?;
    store
        .set_note_protected(id, false)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Best-effort backfill of plaintext titles for protected notes that predate
/// migration v13's title backfill. That migration could only derive a title
/// from PLAINTEXT content, so a note that was already protected (ciphertext)
/// at the time was left with `title = ''` (see `migrate.rs`, schema v13) —
/// "picked up on their next save or re-protect" per that comment. This adds
/// the vault-unlock moment as another chance: called from every unlock path
/// (`vault_unlock`, `vault_unlock_recovery`, `vault_unlock_biometric`) right
/// after the DEK becomes available.
///
/// Best-effort by design: a note whose content fails to decrypt (corrupt
/// blob, foreign/mismatched key) is silently skipped rather than aborting
/// the unlock, and nothing here ever logs key or plaintext material. Only
/// `title` is written — `content` is read but never rewritten, preserving
/// the `content ciphertext ⟺ protected = 1` invariant.
pub fn backfill_protected_titles(store: &Store, dek: &Dek) {
    let ids: Vec<String> = {
        let mut stmt = match store
            .conn
            .prepare("SELECT id FROM notes WHERE protected = 1 AND title = ''")
        {
            Ok(stmt) => stmt,
            Err(_) => return,
        };
        let rows = match stmt.query_map([], |r| r.get::<_, String>(0)) {
            Ok(rows) => rows,
            Err(_) => return,
        };
        rows.filter_map(Result::ok).collect()
    };

    for id in ids {
        let stored = match store.load_note_content(&id) {
            Ok(Some(c)) => c,
            _ => continue,
        };
        let plaintext = match open_content(dek, &id, &stored) {
            Ok(p) => p,
            Err(_) => continue, // can't decrypt (corrupt/foreign) — skip, never abort the unlock
        };
        let title = crate::storage::note_preview(&plaintext);
        if store.set_title(&id, &title).is_ok() {
            // Propagate the freshly-derived title to other devices — otherwise
            // the server keeps the empty title until the note is next edited.
            // Only touches notes with an empty title (a one-time set), so this
            // can't cause a recurring sync churn.
            let _ = store.mark_note_dirty_if_syncing(&id);
        }
    }
}

// ---------------------------------------------------------------------------
// Notes
// ---------------------------------------------------------------------------

/// `notes_load_one`: the full HTML content of one note (empty string if it no
/// longer exists). Protected notes require an unlocked vault — `dek: None`
/// yields `Err("vault locked")` — and are decrypted before returning.
pub fn load_note_content(store: &Store, dek: Option<&Dek>, id: &str) -> Result<String, String> {
    let stored = match store.load_note_content(id).map_err(|e| e.to_string())? {
        Some(c) => c,
        None => return Ok(String::new()),
    };
    if store.note_protected(id).map_err(|e| e.to_string())? {
        let dek = dek.ok_or_else(|| "vault locked".to_string())?;
        open_content(dek, id, &stored)
    } else {
        Ok(stored)
    }
}

/// `notes_search`: full-text search within one context (title-first), with
/// snippets. Protected notes are excluded while the vault is locked — their
/// `content` is ciphertext, so a plaintext scan can't match it correctly anyway.
pub fn search_notes(
    store: &Store,
    query: &str,
    vault_unlocked: bool,
) -> Result<Vec<SearchHit>, String> {
    store
        .search_notes(query, 50, !vault_unlocked)
        .map_err(|e| e.to_string())
}

/// `notes_search_all`: the same search across every context, tagged with its
/// context. The vault is a single local vault shared by every context, so the
/// same lock state gates protected rows everywhere.
pub fn search_all_contexts(
    contexts: &[crate::aggregate::Ctx],
    query: &str,
    vault_unlocked: bool,
) -> Vec<crate::aggregate::TaggedHit> {
    crate::aggregate::search_all(contexts, query, 50, !vault_unlocked)
}

/// `notes_save`. The title is always captured from the PLAINTEXT html the
/// editor sends — before either branch might seal `content` into ciphertext.
/// The title is deliberately visible metadata even for a protected note (only
/// the body is secret), so it must never be derived from the sealed content.
///
/// A note that is effectively protected (its own flag or a locked ancestor
/// folder) is refused with `Err("vault locked")` when `dek` is `None`, is
/// stored as ciphertext, and never contributes a plaintext revision — any
/// revisions recorded before the transition are purged on every protected save.
pub fn save_note(store: &Store, dek: Option<&Dek>, note: &Note) -> Result<(), String> {
    let title = crate::storage::note_preview(&note.content);
    let protected = store
        .is_effectively_protected(&note.id)
        .map_err(|e| e.to_string())?;
    if protected {
        let dek = dek.ok_or_else(|| "vault locked".to_string())?;
        let mut sealed = note.clone();
        sealed.content = seal_content(dek, &note.id, &note.content);
        store.save_note(&sealed).map_err(|e| e.to_string())?;
        store
            .set_title(&note.id, &title)
            .map_err(|e| e.to_string())?;
        store
            .set_note_protected(&note.id, true)
            .map_err(|e| e.to_string())?;
        // Never persist a protected note's plaintext into the (unencrypted)
        // note_revisions table — skip revision history for this save, and
        // purge any plaintext revisions recorded before this transition
        // (no-op if already empty; safe to call on every protected save).
        crate::revisions::delete_revisions(&store.conn, &note.id).map_err(|e| e.to_string())?;
    } else {
        store.save_note(note).map_err(|e| e.to_string())?;
        store
            .set_title(&note.id, &title)
            .map_err(|e| e.to_string())?;
        let limit = crate::settings::get_int(&store.conn, "revisionLimit", 50);
        crate::revisions::add_revision(&store.conn, &note.id, &note.content, limit)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// `notes_delete`: three-way policy. A syncing context tombstones the note so
/// the delete propagates; otherwise the note goes to the trash when
/// `trashEnabled` (default on), or is deleted outright when it is off.
pub fn delete_note(store: &Store, id: &str, now: i64) -> Result<(), String> {
    if store.sync_enabled {
        store.sync_delete_note(id).map_err(|e| e.to_string())
    } else if crate::settings::get_bool_default(&store.conn, "trashEnabled", true) {
        store.trash_note(id, now).map_err(|e| e.to_string())
    } else {
        store.delete_note(id).map_err(|e| e.to_string())
    }
}

/// `note_stats`: aggregate counters over every (non-trashed) note.
pub fn note_stats(store: &Store) -> Result<crate::stats::Stats, String> {
    let notes = store.load_notes().map_err(|e| e.to_string())?;
    Ok(crate::stats::compute(&notes))
}

// ---------------------------------------------------------------------------
// Folders
// ---------------------------------------------------------------------------

/// A folder mutation in a syncing context has to bump `updated_at` + `dirty`
/// so it is pushed; in a local context the row is left alone.
fn touch_folder_if_syncing(store: &Store, id: &str) -> Result<(), String> {
    if store.sync_enabled {
        crate::folders::touch_folder(&store.conn, id).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// `folder_create`.
pub fn folder_create(
    store: &Store,
    id: &str,
    name: &str,
    parent_id: Option<&str>,
) -> Result<(), String> {
    crate::folders::create_folder(&store.conn, id, name, parent_id).map_err(|e| e.to_string())?;
    touch_folder_if_syncing(store, id)
}

/// `folder_rename`.
pub fn folder_rename(store: &Store, id: &str, name: &str) -> Result<(), String> {
    crate::folders::rename_folder(&store.conn, id, name).map_err(|e| e.to_string())?;
    touch_folder_if_syncing(store, id)
}

/// `folder_move`: re-parents a folder (`None` = root).
pub fn folder_move(store: &Store, id: &str, parent_id: Option<&str>) -> Result<(), String> {
    crate::folders::move_folder(&store.conn, id, parent_id).map_err(|e| e.to_string())?;
    touch_folder_if_syncing(store, id)
}

/// `folder_delete`: a syncing context tombstones the folder (and, per `mode`,
/// its subtree) so the delete propagates; a local one removes the rows.
pub fn folder_delete(store: &Store, id: &str, mode: &str) -> Result<(), String> {
    let mode = crate::folders::DeleteMode::from_str(mode);
    if store.sync_enabled {
        crate::folders::sync_delete_folder(&store.conn, id, mode).map_err(|e| e.to_string())
    } else {
        crate::folders::delete_folder(&store.conn, id, mode).map_err(|e| e.to_string())
    }
}

/// `folders_reorder`.
pub fn folders_reorder(
    store: &Store,
    parent_id: Option<&str>,
    ids: &[String],
) -> Result<(), String> {
    crate::folders::reorder_folders(&store.conn, parent_id, ids).map_err(|e| e.to_string())
}

/// `folder_set_icon`.
pub fn folder_set_icon(store: &Store, id: &str, icon: &str) -> Result<(), String> {
    crate::folders::set_folder_icon(&store.conn, id, icon).map_err(|e| e.to_string())?;
    touch_folder_if_syncing(store, id)
}

/// `folder_set_color`.
pub fn folder_set_color(store: &Store, id: &str, color: &str) -> Result<(), String> {
    crate::folders::set_folder_color(&store.conn, id, color).map_err(|e| e.to_string())?;
    touch_folder_if_syncing(store, id)
}

/// `folder_set_sort`.
pub fn folder_set_sort(store: &Store, id: &str, sort: &str) -> Result<(), String> {
    crate::folders::set_folder_sort(&store.conn, id, sort).map_err(|e| e.to_string())?;
    touch_folder_if_syncing(store, id)
}

/// Core reconciliation logic behind `notes_set_folder`: `dek` is `None` to
/// represent a locked vault, `Some(&dek)` unlocked.
///
/// Moves only ever ADD protection, never remove it:
/// - Already-encrypted notes stay encrypted regardless of destination — the
///   safe direction; moving an encrypted note out of a locked folder does
///   NOT auto-decrypt it (the user can explicitly unprotect it).
/// - A currently-plaintext note moving into a location with a locked
///   ancestor folder must become encrypted. That check — and the
///   `dek.is_some()` requirement it implies — happens BEFORE the move is
///   performed, so a locked vault never leaves the note relocated into a
///   locked folder while still plaintext: this returns `Err("vault
///   locked")` and the note stays where (and as) it was.
/// - A plaintext note moving into a non-locked location is an unchanged
///   plain move.
/// - The freshly-encrypted note's revision history is purged (it was
///   plaintext, so no plaintext survives a transition to encrypted).
pub fn reconcile_folder_move(
    store: &Store,
    id: &str,
    folder_id: Option<&str>,
    dek: Option<&Dek>,
) -> Result<(), String> {
    let already_protected = store.note_protected(id).map_err(|e| e.to_string())?;
    let needs_encryption = !already_protected && folder_chain_has_lock(store, folder_id)?;

    if needs_encryption {
        let dek = dek.ok_or_else(|| "vault locked".to_string())?;
        store.set_folder(id, folder_id).map_err(|e| e.to_string())?;
        encrypt_note_in_place(store, id, dek)?;
    } else {
        store.set_folder(id, folder_id).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Core reconciliation behind `notes_reorder`, sharing
/// [`reconcile_folder_move`]'s convention: `dek` is `None` for a locked vault,
/// `Some(&dek)` unlocked.
///
/// Drag-and-drop reorder assigns every id to `folder_id`. If that destination
/// has a locked ancestor, any currently-plaintext note among `ids` would land
/// inside a locked subtree as plaintext-at-rest — the same leak
/// [`reconcile_folder_move`] prevents for the context-menu move path. So:
/// - Already-protected (ciphertext) notes just get repositioned — the safe
///   direction, never auto-decrypted.
/// - A plaintext note entering the locked destination must be encrypted with
///   the SAME primitive the move path uses ([`encrypt_note_in_place`]).
/// - If the vault is locked and any plaintext note would enter the locked
///   subtree, the WHOLE operation is refused (`Err("vault locked")`) before any
///   row is touched — never a half-applied reorder that strands plaintext in a
///   locked folder.
/// - A non-locked destination is an unchanged plain reorder.
pub fn reconcile_reorder(
    store: &Store,
    folder_id: Option<&str>,
    ids: &[String],
    dek: Option<&Dek>,
) -> Result<(), String> {
    if !folder_chain_has_lock(store, folder_id)? {
        return store
            .reorder_notes(folder_id, ids)
            .map_err(|e| e.to_string());
    }

    // Destination is inside a locked subtree. Which ids are currently plaintext
    // (physically unencrypted) and therefore need sealing on entry? A missing
    // id (stale drag payload) is skipped — `reorder_notes` no-ops it too.
    use rusqlite::OptionalExtension;
    let mut to_encrypt: Vec<&String> = Vec::new();
    for id in ids {
        let protected: Option<bool> = store
            .conn
            .query_row("SELECT protected FROM notes WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .optional()
            .map_err(|e| e.to_string())?;
        if protected == Some(false) {
            to_encrypt.push(id);
        }
    }

    // Refuse the entire op up front if we'd strand a plaintext note in a locked
    // folder without an unlocked DEK — nothing is mutated on this path.
    if !to_encrypt.is_empty() && dek.is_none() {
        return Err("vault locked".to_string());
    }

    store
        .reorder_notes(folder_id, ids)
        .map_err(|e| e.to_string())?;
    if let Some(dek) = dek {
        for id in to_encrypt {
            encrypt_note_in_place(store, id, dek)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Vault
// ---------------------------------------------------------------------------

/// Loads and parses the persisted vault record, or a "not set up" error if
/// none exists yet.
pub fn load_vault_record(store: &Store) -> Result<VaultRecord, String> {
    let json = store.vault_record().map_err(|e| e.to_string())?;
    let json = json.ok_or_else(|| "vault: not set up".to_string())?;
    VaultRecord::from_json(&json).map_err(String::from)
}

/// `vault_setup`: create a brand-new vault — wrap a fresh DEK under
/// `passphrase`, persist the record, and hand back the one-time recovery key
/// (split into its dash-separated groups) plus the DEK so the caller can arm
/// the session without a redundant unlock.
///
/// Guards against clobbering an existing vault: a second setup would generate
/// a brand-new DEK and overwrite the stored record, orphaning the old DEK and
/// permanently losing any notes already encrypted under it. This sits behind
/// the Tauri command (the trust boundary), so the check has to live here
/// rather than relying on the frontend to gate it. Nothing is generated or
/// persisted until we know no record exists.
pub fn vault_setup(store: &Store, passphrase: &str) -> Result<(Vec<String>, Dek), String> {
    if store.vault_record().map_err(|e| e.to_string())?.is_some() {
        return Err("vault: a vault already exists".to_string());
    }
    let (record, recovery_key, dek) = crate::vault::setup(passphrase).map_err(String::from)?;
    store
        .set_vault_record(&record.to_json())
        .map_err(|e| e.to_string())?;
    Ok((
        recovery_key.as_str().split('-').map(String::from).collect(),
        dek,
    ))
}

/// `vault_unlock`: derive the KEK from `passphrase` and unwrap the DEK.
///
/// Takes an already-loaded record rather than the `Store`: these two unlock
/// paths are read-only, so — unlike [`vault_setup`] and
/// [`vault_change_passphrase`] — there is no check-then-write to make atomic,
/// and the caller must NOT hold the store lock across the (~0.5 s) Argon2
/// derivation. Doing so would stall autosave, the sync cycle and the MCP
/// server for the duration.
pub fn vault_unlock_passphrase(record: &VaultRecord, passphrase: &str) -> Result<Dek, String> {
    crate::vault::unlock_passphrase(record, passphrase).map_err(String::from)
}

/// `vault_unlock_recovery`: same, but via the one-time recovery key (accepts
/// any formatting the user typed — separators and case are normalized). Takes
/// an already-loaded record for the same reason as
/// [`vault_unlock_passphrase`].
pub fn vault_unlock_recovery(record: &VaultRecord, recovery: &str) -> Result<Dek, String> {
    crate::vault::unlock_recovery(record, recovery).map_err(String::from)
}

/// `vault_change_passphrase`: unlock with `current`, re-wrap the SAME DEK
/// under `next`, and persist the updated record. The existing recovery key
/// keeps working — `rewrap_passphrase` never touches its wrapping.
///
/// Returns the (unchanged) DEK so the caller can re-arm the session: `current`
/// was just cryptographically re-verified, so that is safe rather than forcing
/// a redundant unlock.
/// Self-heal: a record written before the DEK check existed gains one the
/// first time its owner proves possession of the DEK via passphrase or
/// recovery key. Best-effort — a failed write only means the next unlock
/// tries again; it never fails the unlock itself.
pub fn ensure_dek_check(store: &Store, record: &VaultRecord, dek: &Dek) {
    if record.dek_check.is_some() {
        return;
    }
    let upgraded = VaultRecord {
        kdf_params: record.kdf_params.clone(),
        dek_wrapped_pass: record.dek_wrapped_pass.clone(),
        recovery_salt: record.recovery_salt,
        dek_wrapped_recovery: record.dek_wrapped_recovery.clone(),
        dek_check: Some(crate::vault::make_dek_check(dek)),
    };
    let _ = store.set_vault_record(&upgraded.to_json());
}

/// Gate for DEKs that arrive WITHOUT a proof of ownership — today the
/// biometric keychain item. Refuses a DEK that doesn't open this vault's
/// check (a key from another context), and refuses to guess when the record
/// has no check yet: that record must first be unlocked with its passphrase
/// or recovery key once, which writes the check (`ensure_dek_check`).
pub fn verify_dek_for_store(store: &Store, dek: &Dek) -> Result<(), String> {
    let record = load_vault_record(store)?;
    match crate::vault::verify_dek(&record, dek) {
        Ok(true) => Ok(()),
        Ok(false) => Err(
            "vault: unlock with your passphrase once to finish upgrading this vault, then re-enable biometric unlock"
                .to_string(),
        ),
        Err(_) => Err("vault: biometric key belongs to a different context".to_string()),
    }
}

pub fn vault_change_passphrase(store: &Store, current: &str, next: &str) -> Result<Dek, String> {
    let record = load_vault_record(store)?;
    let dek = crate::vault::unlock_passphrase(&record, current).map_err(String::from)?;
    let new_record = crate::vault::rewrap_passphrase(&record, &dek, next);
    store
        .set_vault_record(&new_record.to_json())
        .map_err(|e| e.to_string())?;
    Ok(dek)
}

/// `note_set_protected`: encrypts or decrypts one note's stored content in
/// place, keeping `notes.protected` in sync with the physical content state.
/// Requires an unlocked vault (`dek`) — the command refuses with
/// `Err("vault locked")` before ever reaching here.
///
/// `protected = false` is refused while the note is inside a `locked` folder —
/// the folder is the source of truth for that note's protection until the
/// folder itself is unlocked.
///
/// Transitioning to `protected = true` discards the note's existing revision
/// history (see [`encrypt_note_in_place`]) — v1 behavior, since
/// `note_revisions` is unencrypted.
pub fn set_note_protected(
    store: &Store,
    dek: &Dek,
    id: &str,
    protected: bool,
) -> Result<(), String> {
    if protected {
        if !store.note_protected(id).map_err(|e| e.to_string())? {
            // Seal + flip `protected` + mark dirty + purge the plaintext
            // revision history (v1: keeping it would defeat
            // encryption-at-rest, since note_revisions is unencrypted).
            encrypt_note_in_place(store, id, dek)?;
        }
    } else {
        if has_locked_ancestor_folder(store, id)? {
            return Err("note is protected by its folder".to_string());
        }
        if store.note_protected(id).map_err(|e| e.to_string())? {
            decrypt_note_in_place(store, id, dek)?;
        }
    }
    Ok(())
}

/// `folder_set_locked`: locks or unlocks a folder, encrypting/decrypting the
/// notes in its subtree to match. Requires an unlocked vault (`dek`).
///
/// v1 limitation: `notes.protected` tracks only physical ciphertext state,
/// not a separate "individually locked" intent, so unlocking a folder
/// decrypts every subtree note that has no *other* locked ancestor —
/// including a note that was individually protected while it happened to
/// live inside this now-unlocking folder. Acceptable for v1.
///
/// Locking (not unlocking) also discards each newly-encrypted note's
/// existing revision history, same rationale as [`set_note_protected`].
pub fn set_folder_locked(store: &Store, dek: &Dek, id: &str, locked: bool) -> Result<(), String> {
    let note_ids = store.note_ids_in_subtree(id).map_err(|e| e.to_string())?;

    if locked {
        store
            .set_folder_locked(id, true)
            .map_err(|e| e.to_string())?;
        for note_id in &note_ids {
            if !store.note_protected(note_id).map_err(|e| e.to_string())? {
                // Same transition as set_note_protected(id, true): seal +
                // flip `protected` + mark dirty + discard this note's now
                // encryption-defeating plaintext revision history.
                encrypt_note_in_place(store, note_id, dek)?;
            }
        }
    } else {
        store
            .set_folder_locked(id, false)
            .map_err(|e| e.to_string())?;
        for note_id in &note_ids {
            let still_locked = has_locked_ancestor_folder(store, note_id)?;
            if store.note_protected(note_id).map_err(|e| e.to_string())? && !still_locked {
                decrypt_note_in_place(store, note_id, dek)?;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Context registry
// ---------------------------------------------------------------------------

/// Registry snapshot for the frontend context switcher.
pub fn to_infos(reg: &Registry) -> Vec<ContextInfo> {
    reg.contexts
        .iter()
        .map(|c| ContextInfo {
            id: c.id.clone(),
            label: c.label.clone(),
            kind: c.kind.clone(),
            path: c.path.clone(),
            server_url: c.server_url.clone(),
            workspace_id: c.workspace_id.clone(),
            active: c.id == reg.active_id,
        })
        .collect()
}

/// Snapshot the registry's contexts as aggregator `Ctx` descriptors
/// (`notes_load_all` / `notes_search_all`).
pub fn registry_contexts(reg: &Registry) -> Vec<crate::aggregate::Ctx> {
    reg.contexts
        .iter()
        .map(|c| crate::aggregate::Ctx {
            id: c.id.clone(),
            label: c.label.clone(),
            kind: c.kind.clone(),
            path: c.path.clone(),
        })
        .collect()
}

/// The active context, but only if it is server-backed.
pub fn active_server(reg: &Registry) -> Option<ContextEntry> {
    reg.active().filter(|c| c.kind == "server").cloned()
}

/// Create a context's own directory (so its images, resolved as
/// `<db-dir>/images`, stay isolated from every other context) and initialise
/// the database inside it. Returns the DB path.
pub fn prepare_context_db(contexts_dir: &Path, id: &str) -> Result<PathBuf, String> {
    let dir = contexts_dir.join(id);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("notefix.db");
    let s = Store::open(&path).map_err(|e| e.to_string())?;
    crate::migrate::run_migrations(&s.conn).map_err(|e| e.to_string())?;
    Ok(path)
}

/// `context_add`: create a fresh local context, make it active, and persist the
/// registry. Returns its DB path (for the store swap) and the new snapshot.
pub fn context_add(
    reg: &mut Registry,
    contexts_dir: &Path,
    profiles_path: &Path,
    label: String,
) -> Result<(PathBuf, Vec<ContextInfo>), String> {
    let id = uuid::Uuid::new_v4().to_string();
    let path = prepare_context_db(contexts_dir, &id)?;
    reg.add(id.clone(), label, path.to_string_lossy().into_owned());
    reg.set_active(&id)?;
    crate::profiles::save(profiles_path, reg).map_err(|e| e.to_string())?;
    Ok((path, to_infos(reg)))
}

/// `context_switch`: make `id` active and persist. Returns its DB path and
/// whether it is server-backed (i.e. whether the swapped store syncs).
pub fn context_switch(
    reg: &mut Registry,
    profiles_path: &Path,
    id: &str,
) -> Result<(String, bool), String> {
    reg.set_active(id)?;
    let path = reg
        .active()
        .map(|c| c.path.clone())
        .ok_or_else(|| "unknown context".to_string())?;
    let kind = reg.active().map(|c| c.kind.clone()).unwrap_or_default();
    crate::profiles::save(profiles_path, reg).map_err(|e| e.to_string())?;
    Ok((path, kind == "server"))
}

/// `context_rename`.
pub fn context_rename(
    reg: &mut Registry,
    profiles_path: &Path,
    id: &str,
    label: String,
) -> Result<Vec<ContextInfo>, String> {
    reg.rename(id, label)?;
    crate::profiles::save(profiles_path, reg).map_err(|e| e.to_string())?;
    Ok(to_infos(reg))
}

/// `context_remove`: drop the context from the registry, persist, and — when
/// `delete_file` — remove its database (plus the `-wal`/`-shm` sidecars).
/// Returns the removed entry so the caller can clear a server context's
/// keychain tokens, and the new snapshot.
pub fn context_remove(
    reg: &mut Registry,
    profiles_path: &Path,
    id: &str,
    delete_file: bool,
) -> Result<(ContextEntry, Vec<ContextInfo>), String> {
    let removed = reg.remove(id)?;
    crate::profiles::save(profiles_path, reg).map_err(|e| e.to_string())?;
    let infos = to_infos(reg);
    if delete_file {
        for ext in ["", "-wal", "-shm"] {
            let p = with_ext(Path::new(&removed.path), ext);
            let _ = std::fs::remove_file(p);
        }
    }
    Ok((removed, infos))
}

/// `context_bind_workspace`: bind a server context to a workspace and
/// optionally rename it in the same step.
pub fn context_bind_workspace(
    reg: &mut Registry,
    profiles_path: &Path,
    id: &str,
    workspace_id: String,
    label: String,
) -> Result<Vec<ContextInfo>, String> {
    reg.bind_workspace(id, workspace_id)?;
    if !label.is_empty() {
        reg.rename(id, label)?;
    }
    crate::profiles::save(profiles_path, reg).map_err(|e| e.to_string())?;
    Ok(to_infos(reg))
}

/// Register a freshly authenticated server context as the active one and
/// persist the registry (`server_auth_complete`).
pub fn register_server_context(
    reg: &mut Registry,
    profiles_path: &Path,
    id: &str,
    label: String,
    path: &Path,
    server_url: String,
) -> Result<Vec<ContextInfo>, String> {
    reg.add_server(
        id.to_string(),
        label,
        path.to_string_lossy().into_owned(),
        server_url,
    );
    reg.set_active(id)?;
    crate::profiles::save(profiles_path, reg).map_err(|e| e.to_string())?;
    Ok(to_infos(reg))
}

// ---------------------------------------------------------------------------
// Browser auth flow bookkeeping
// ---------------------------------------------------------------------------

/// A server context's display label: the host of its URL, falling back to the
/// raw URL when it can't be parsed.
pub fn server_label(server_url: &str) -> String {
    url::Url::parse(server_url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_else(|| server_url.to_string())
}

/// `server_auth_begin`: build the browser authorize URL from the discovered
/// OAuth config plus freshly minted PKCE material.
pub fn build_authorize_url(
    config: &crate::auth::OAuthConfig,
    challenge: &str,
    state: &str,
) -> Result<String, String> {
    let mut authorize = url::Url::parse(&config.authorize_url).map_err(|e| e.to_string())?;
    {
        let mut q = authorize.query_pairs_mut();
        q.append_pair("response_type", "code");
        q.append_pair("client_id", &config.client_id);
        q.append_pair("redirect_uri", crate::auth::REDIRECT_URI);
        q.append_pair("code_challenge", challenge);
        q.append_pair("code_challenge_method", "S256");
        q.append_pair("state", state);
        if !config.scopes.is_empty() {
            q.append_pair("scope", &config.scopes.join(" "));
        }
    }
    Ok(authorize.to_string())
}

/// `server_auth_complete` step 1: pull `code` + `state` out of the
/// `notefix://auth?code=…&state=…` callback URL.
pub fn parse_auth_callback(url: &str) -> Result<(String, String), String> {
    let parsed = url::Url::parse(url).map_err(|e| e.to_string())?;
    let (mut code, mut state) = (None, None);
    for (k, v) in parsed.query_pairs() {
        match k.as_ref() {
            "code" => code = Some(v.into_owned()),
            "state" => state = Some(v.into_owned()),
            _ => {}
        }
    }
    let code = code.ok_or("missing code in callback")?;
    let state = state.ok_or("missing state in callback")?;
    Ok((code, state))
}

// ---------------------------------------------------------------------------
// Sync status
// ---------------------------------------------------------------------------

/// The part of `sync_status` that needs only the registry: a context that is
/// not server-backed reports `"local"`, a server context with no workspace
/// bound yet reports `"unbound"`. `None` means the context is bound and the
/// store has to be consulted — see [`sync_status_synced`].
pub fn sync_status_local(reg: &Registry) -> Option<SyncStatus> {
    match active_server(reg) {
        None => Some(SyncStatus {
            state: "local".into(),
            last_synced_at: 0,
            pending: 0,
        }),
        Some(ctx) if ctx.workspace_id.is_empty() => Some(SyncStatus {
            state: "unbound".into(),
            last_synced_at: 0,
            pending: 0,
        }),
        Some(_) => None,
    }
}

/// The rest of `sync_status` for a bound server context: `"synced"` once a
/// cycle has completed, `"syncing"` before the first one, plus the number of
/// dirty notes + folders still waiting to be pushed.
pub fn sync_status_synced(store: &Store) -> Result<SyncStatus, String> {
    let last = crate::migrate::get_meta_i64(&store.conn, "sync_last_at", 0);
    let pending = store.load_dirty_notes().map_err(|e| e.to_string())?.len() as i64
        + crate::folders::load_dirty_folders(&store.conn)
            .map_err(|e| e.to_string())?
            .len() as i64;
    let state = if last > 0 { "synced" } else { "syncing" };
    Ok(SyncStatus {
        state: state.into(),
        last_synced_at: last,
        pending,
    })
}

/// The snapshot `run_sync_cycle` takes under the store lock before its network
/// round-trip. Kept as one value so the push payload and the ids needed to
/// clear the dirty flags afterwards can never drift apart.
pub struct SyncPush {
    /// Dirty folders, already mapped to the server wire shape.
    pub folders: Vec<serde_json::Value>,
    /// Dirty notes, already mapped to the server wire shape.
    pub notes: Vec<serde_json::Value>,
    /// Snapshot of `(id, updated_at)` per pushed note, so the post-sync
    /// dirty-clear skips any row re-edited during the network window — its
    /// `updated_at` will have moved on, and it stays queued for the next cycle
    /// instead of being silently dropped.
    pub note_ids: Vec<(String, i64)>,
    /// Same snapshot for folders.
    pub folder_ids: Vec<(String, i64)>,
    /// The pull cursor to resume from.
    pub since: i64,
}

/// Collect everything `run_sync_cycle` needs before releasing the store lock
/// for its network calls.
pub fn collect_sync_push(store: &Store) -> Result<SyncPush, String> {
    let dn = store.load_dirty_notes().map_err(|e| e.to_string())?;
    let df = crate::folders::load_dirty_folders(&store.conn).map_err(|e| e.to_string())?;
    let since = crate::migrate::get_meta_i64(&store.conn, "sync_cursor", 0);
    Ok(SyncPush {
        folders: df.iter().map(crate::sync::folder_to_wire).collect(),
        notes: dn.iter().map(crate::sync::note_to_wire).collect(),
        note_ids: dn.iter().map(|n| (n.id.clone(), n.updated_at)).collect(),
        folder_ids: df.iter().map(|f| (f.id.clone(), f.updated_at)).collect(),
        since,
    })
}

/// Apply one successful sync round-trip: clear the dirty flags for exactly the
/// rows that were pushed and are still unchanged, merge the pulled rows, and
/// advance both sync markers. `note_ids` / `folder_ids` are the snapshots from
/// the matching [`SyncPush`].
pub fn commit_sync_result(
    store: &Store,
    note_ids: &[(String, i64)],
    folder_ids: &[(String, i64)],
    pulled_folders: &[serde_json::Value],
    pulled_notes: &[serde_json::Value],
    cursor: i64,
    now: i64,
) -> Result<(), String> {
    store
        .clear_note_dirty(note_ids)
        .map_err(|e| e.to_string())?;
    crate::folders::clear_folder_dirty(&store.conn, folder_ids).map_err(|e| e.to_string())?;
    crate::sync::apply_pulled(store, pulled_folders, pulled_notes).map_err(|e| e.to_string())?;
    crate::migrate::set_meta_i64(&store.conn, "sync_cursor", cursor).map_err(|e| e.to_string())?;
    crate::migrate::set_meta_i64(&store.conn, "sync_last_at", now).map_err(|e| e.to_string())?;
    Ok(())
}

/// Of the image paths a context references, the subset that actually exists in
/// its images folder. A path that fails `safe_subpath` validation counts as
/// absent, so a malicious relpath can never make the image phase read outside
/// the images root.
pub fn locally_present_images(
    referenced: &std::collections::HashSet<String>,
    images_root: &Path,
) -> std::collections::HashSet<String> {
    referenced
        .iter()
        .filter(|p| {
            crate::images::safe_subpath(p)
                .map(|sp| images_root.join(sp).is_file())
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Export / import / images
// ---------------------------------------------------------------------------

/// Empty `ids` means "everything"; otherwise keep only the selected notes,
/// preserving the input order.
pub fn select_notes(notes: Vec<Note>, ids: &[String]) -> Vec<Note> {
    if ids.is_empty() {
        notes
    } else {
        notes.into_iter().filter(|n| ids.contains(&n.id)).collect()
    }
}

/// Read one image referenced from note HTML, as `(mime, bytes)` — `None` for
/// an unsafe relative path or a missing file, which leaves the URL untouched.
fn read_image(images_root: &Path, rel: &str) -> Option<(String, Vec<u8>)> {
    let safe = crate::images::safe_subpath(rel)?;
    let bytes = std::fs::read(images_root.join(&safe)).ok()?;
    Some((crate::images::mime_for(rel).to_string(), bytes))
}

/// `export_notes`: plain JSON export (images stay as `noteimg://` URLs).
///
/// Takes the already-loaded notes rather than the `Store`, so the caller can
/// release the store lock before this writes to disk — a slow or unreachable
/// destination must not pin the database (and with it autosave, sync and the
/// MCP server). Same reason for the three ops below.
pub fn export_notes_json(notes: &[Note], path: &Path, ids: &[String]) -> Result<(), String> {
    let json = crate::export::notes_to_json(notes, ids).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

/// `export_notes_base64`: JSON export with every referenced image inlined as a
/// `data:` URL, so the file is self-contained.
pub fn export_notes_inlined(
    notes: Vec<Note>,
    images_root: &Path,
    path: &Path,
    ids: &[String],
) -> Result<(), String> {
    let out: Vec<Note> = select_notes(notes, ids)
        .into_iter()
        .map(|mut n| {
            n.content =
                crate::export::inline_images(&n.content, |rel| read_image(images_root, rel));
            n
        })
        .collect();
    let json = serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

/// `note_inlined_html`: one note's HTML with its images inlined as `data:`
/// URLs (used for print/PDF). `notes` is expected to be `load_all_notes()`, so
/// a note that was just trashed in another window can still be printed.
pub fn note_inlined_html(
    notes: Vec<Note>,
    images_root: &Path,
    note_id: &str,
) -> Result<String, String> {
    let note = notes
        .into_iter()
        .find(|n| n.id == note_id)
        .ok_or_else(|| "note not found".to_string())?;
    Ok(crate::export::inline_images(&note.content, |rel| {
        read_image(images_root, rel)
    }))
}

/// `save_export`: write frontend-produced bytes to the path the user picked.
pub fn save_export(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

/// Copy the images a bundle references into `<dest>/images/`, preserving their
/// relative layout. Unsafe relative paths are skipped; a missing source file is
/// non-fatal (the bundle simply lacks that image).
fn copy_bundle_images(images_root: &Path, dest: &Path, paths: &[String]) {
    for rel in paths {
        if let Some(safe) = crate::images::safe_subpath(rel) {
            let to = dest.join("images").join(&safe);
            if let Some(parent) = to.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::copy(images_root.join(&safe), &to);
        }
    }
}

/// `export_md_bundle`: one Markdown file plus its `images/` folder. Path
/// separators and `:` in `name` are replaced so the filename stays valid.
pub fn export_md_bundle(
    images_root: &Path,
    dest: &Path,
    md: &str,
    name: &str,
) -> Result<(), String> {
    let (rewritten, paths) = crate::export::to_bundle(md);
    std::fs::create_dir_all(dest.join("images")).map_err(|e| e.to_string())?;
    copy_bundle_images(images_root, dest, &paths);
    let fname = format!("{}.md", name.replace(['/', '\\', ':'], "-"));
    std::fs::write(dest.join(fname), rewritten).map_err(|e| e.to_string())
}

/// `export_notes_bundle`: `notes.json` plus an `images/` folder, with every
/// `noteimg://` URL rewritten to a bundle-relative `images/…` path.
pub fn export_notes_bundle(
    notes: Vec<Note>,
    images_root: &Path,
    dest: &Path,
    ids: &[String],
) -> Result<(), String> {
    std::fs::create_dir_all(dest.join("images")).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for mut n in select_notes(notes, ids) {
        let (content, paths) = crate::export::to_bundle(&n.content);
        copy_bundle_images(images_root, dest, &paths);
        n.content = content;
        out.push(n);
    }
    let json = serde_json::to_string_pretty(&out).map_err(|e| e.to_string())?;
    std::fs::write(dest.join("notes.json"), json).map_err(|e| e.to_string())
}

/// `save_image`: write a pasted/dropped image under the note's sharded images
/// directory and return the `noteimg://` URL that references it. Both the file
/// name and the note id are validated as safe relative subpaths, so neither can
/// escape the images root.
pub fn save_image(
    images_root: &Path,
    note_id: &str,
    name: &str,
    bytes: &[u8],
) -> Result<String, String> {
    let name = crate::images::safe_subpath(name).ok_or_else(|| "invalid name".to_string())?;
    let sub = crate::images::safe_subpath(&crate::images::shard(note_id))
        .ok_or_else(|| "invalid note id".to_string())?;
    let dir = images_root.join(&sub);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(&name), bytes).map_err(|e| e.to_string())?;
    Ok(crate::images::note_image_url(note_id, &name))
}

/// `check_paths`: the storage diagnostics shown in Settings — whether the
/// database's directory and the images folder are actually writable.
pub fn check_paths(db_path: &Path, images: &Path) -> PathChecks {
    let db_dir = db_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();
    PathChecks {
        db_writable: crate::syscheck::is_writable(&db_dir),
        images_writable: crate::syscheck::is_writable(images),
        db_path: db_dir.to_string_lossy().to_string(),
        images_path: images.to_string_lossy().to_string(),
    }
}

// ---------------------------------------------------------------------------
// Storage location
// ---------------------------------------------------------------------------

/// `<path><ext>` — used for the SQLite sidecars (`-wal`, `-shm`); an empty
/// `ext` returns the path unchanged.
pub fn with_ext(path: &Path, ext: &str) -> PathBuf {
    if ext.is_empty() {
        path.to_path_buf()
    } else {
        PathBuf::from(format!("{}{}", path.to_string_lossy(), ext))
    }
}

/// Rename, falling back to copy+delete when the two paths live on different
/// filesystems (which is exactly what a "move my notes to an external drive"
/// relocation does).
fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            std::fs::copy(from, to)?;
            std::fs::remove_file(from)
        }
    }
}

/// Move a database and its `-wal`/`-shm` sidecars from `current` to `target`.
/// The caller must have released the SQLite connection first.
pub fn move_db_files(current: &Path, target: &Path) -> Result<(), String> {
    for ext in ["", "-wal", "-shm"] {
        let from = with_ext(current, ext);
        if from.exists() {
            move_file(&from, &with_ext(target, ext)).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Point an already-managed `Store` at another database file, running
/// migrations so the running app stays consistent until relaunch.
pub fn reopen_store_at(store: &mut Store, path: &Path) -> Result<(), String> {
    store.conn = rusqlite::Connection::open(path).map_err(|e| e.to_string())?;
    crate::migrate::run_migrations(&store.conn).map_err(|e| e.to_string())
}

/// Keep the active context's registry entry pointing at a relocated database.
pub fn point_active_context_at(reg: &mut Registry, path: &Path) {
    let active = reg.active_id.clone();
    if let Some(c) = reg.contexts.iter_mut().find(|c| c.id == active) {
        c.path = path.to_string_lossy().into_owned();
    }
}

// ===========================================================================
// Tests
//
// Every op is exercised against a real (in-memory or temp-file) database, so
// the assertions are about observable state — stored ciphertext/plaintext,
// `protected`/`locked` flags, dirty rows, revision history, files on disk,
// the persisted registry — not just "it returned Ok".
// ===========================================================================

#[cfg(test)]
mod test_support {
    use super::*;

    /// A migrated in-memory store (local context: `sync_enabled = false`).
    pub fn store() -> Store {
        let s = Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        s
    }

    /// A migrated in-memory store standing in for a server-backed context.
    pub fn syncing_store() -> Store {
        let mut s = store();
        s.sync_enabled = true;
        s
    }

    pub fn note(id: &str, content: &str) -> Note {
        Note {
            id: id.into(),
            content: content.into(),
            updated_at: 1,
            ..Default::default()
        }
    }

    /// Insert a plaintext note at the root.
    pub fn seed(s: &Store, id: &str, content: &str) {
        s.save_note(&note(id, content)).unwrap();
    }

    /// Insert a plaintext note directly inside `folder_id`.
    pub fn seed_in(s: &Store, id: &str, content: &str, folder_id: &str) {
        let mut n = note(id, content);
        n.folder_id = Some(folder_id.into());
        s.save_note(&n).unwrap();
    }

    pub fn folder(s: &Store, id: &str, parent: Option<&str>) {
        crate::folders::create_folder(&s.conn, id, id, parent).unwrap();
    }

    /// Clear the dirty flag a sync-enabled save leaves behind, so a test can
    /// prove the op under test is what re-dirties the row.
    pub fn clear_dirty(s: &Store) {
        let rows: Vec<(String, i64)> = s
            .load_dirty_notes()
            .unwrap()
            .into_iter()
            .map(|n| (n.id, n.updated_at))
            .collect();
        s.clear_note_dirty(&rows).unwrap();
        assert!(s.load_dirty_notes().unwrap().is_empty());
    }

    pub fn content_of(s: &Store, id: &str) -> String {
        s.load_note_content(id).unwrap().unwrap()
    }

    pub fn revision_count(s: &Store, id: &str) -> usize {
        crate::revisions::list_revisions(&s.conn, id).unwrap().len()
    }

    /// `Result::unwrap_err` needs `T: Debug`, which `Dek`/`VaultRecord`
    /// deliberately do not implement (a Debug impl on key material is exactly
    /// what must never exist). This gets at the error without that bound.
    /// The persisted vault record, as the commands load it before running the
    /// KDF outside the store lock.
    pub fn record(s: &Store) -> crate::vault::VaultRecord {
        super::load_vault_record(s).unwrap()
    }

    pub fn err_of<T>(r: Result<T, String>) -> String {
        match r {
            Ok(_) => panic!("expected an error, got Ok"),
            Err(e) => e,
        }
    }

    pub fn title_of(s: &Store, id: &str) -> String {
        s.load_notes_meta()
            .unwrap()
            .into_iter()
            .find(|m| m.id == id)
            .map(|m| m.title)
            .unwrap()
    }
}

#[cfg(test)]
mod crypto_tests {
    use super::*;

    #[test]
    fn seal_open_content_roundtrip() {
        let dek = Dek::random();
        let stored = seal_content(&dek, "n1", "<p>secret</p>");
        assert_ne!(stored, "<p>secret</p>");
        assert!(!stored.contains("secret"));
        assert_eq!(open_content(&dek, "n1", &stored).unwrap(), "<p>secret</p>");
    }

    #[test]
    fn sealing_twice_produces_different_ciphertext() {
        // A fresh nonce per seal: identical plaintext must not yield an
        // identical blob, or equal notes would be linkable at rest.
        let dek = Dek::random();
        let a = seal_content(&dek, "n1", "<p>same</p>");
        let b = seal_content(&dek, "n1", "<p>same</p>");
        assert_ne!(a, b);
        assert_eq!(open_content(&dek, "n1", &a).unwrap(), "<p>same</p>");
        assert_eq!(open_content(&dek, "n1", &b).unwrap(), "<p>same</p>");
    }

    #[test]
    fn open_content_rejects_mismatched_note_id() {
        // The note id is bound in as AEAD associated data, so a sealed blob
        // can't be silently opened under a different note's id.
        let dek = Dek::random();
        let stored = seal_content(&dek, "n1", "<p>secret</p>");
        assert!(open_content(&dek, "other-note", &stored).is_err());
    }

    #[test]
    fn open_content_rejects_wrong_key() {
        let stored = seal_content(&Dek::random(), "n1", "<p>secret</p>");
        assert!(open_content(&Dek::random(), "n1", &stored).is_err());
    }

    #[test]
    fn open_content_rejects_non_base64_and_truncated_blobs() {
        let dek = Dek::random();
        assert!(open_content(&dek, "n1", "not base64 ***").is_err());
        let stored = seal_content(&dek, "n1", "<p>secret</p>");
        let truncated = &stored[..stored.len() / 2];
        assert!(open_content(&dek, "n1", truncated).is_err());
        assert!(open_content(&dek, "n1", "").is_err());
    }

    #[test]
    fn open_content_error_never_leaks_key_or_plaintext() {
        let dek = Dek::random();
        let stored = seal_content(&dek, "n1", "<p>the crown jewels</p>");
        let err = open_content(&Dek::random(), "n1", &stored).unwrap_err();
        assert!(!err.contains("crown jewels"));
        assert!(!err.contains(&stored));
    }
}

#[cfg(test)]
mod lock_chain_tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn root_is_never_locked() {
        let s = store();
        assert!(!folder_chain_has_lock(&s, None).unwrap());
    }

    #[test]
    fn unknown_folder_is_not_locked() {
        let s = store();
        assert!(!folder_chain_has_lock(&s, Some("nope")).unwrap());
    }

    #[test]
    fn lock_is_inherited_from_any_ancestor() {
        let s = store();
        folder(&s, "top", None);
        folder(&s, "mid", Some("top"));
        folder(&s, "leaf", Some("mid"));
        assert!(!folder_chain_has_lock(&s, Some("leaf")).unwrap());
        s.set_folder_locked("top", true).unwrap();
        assert!(folder_chain_has_lock(&s, Some("leaf")).unwrap());
        assert!(folder_chain_has_lock(&s, Some("top")).unwrap());
    }

    #[test]
    fn note_without_folder_has_no_locked_ancestor() {
        let s = store();
        seed(&s, "n1", "<p>x</p>");
        assert!(!has_locked_ancestor_folder(&s, "n1").unwrap());
    }

    #[test]
    fn note_inherits_lock_from_its_folder_chain() {
        let s = store();
        folder(&s, "top", None);
        folder(&s, "sub", Some("top"));
        seed_in(&s, "n1", "<p>x</p>", "sub");
        assert!(!has_locked_ancestor_folder(&s, "n1").unwrap());
        s.set_folder_locked("top", true).unwrap();
        assert!(has_locked_ancestor_folder(&s, "n1").unwrap());
    }

    #[test]
    fn missing_note_reports_no_locked_ancestor() {
        let s = store();
        assert!(!has_locked_ancestor_folder(&s, "ghost").unwrap());
    }
}

#[cfg(test)]
mod encrypt_primitive_tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn encrypting_a_note_seals_content_and_flips_protected() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>Secret Title</p><p>body</p>");

        encrypt_note_in_place(&s, "n1", &dek).unwrap();

        let stored = content_of(&s, "n1");
        assert!(!stored.contains("Secret"));
        assert!(s.note_protected("n1").unwrap());
        assert_eq!(
            open_content(&dek, "n1", &stored).unwrap(),
            "<p>Secret Title</p><p>body</p>"
        );
    }

    #[test]
    fn encrypting_captures_the_plaintext_title_before_sealing() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>Secret Title</p><p>body</p>");

        encrypt_note_in_place(&s, "n1", &dek).unwrap();

        let meta = &s.load_notes_meta().unwrap()[0];
        assert_eq!(
            meta.title, "Secret Title",
            "title stays plaintext and findable even though the body is sealed"
        );
        assert_eq!(
            meta.preview, "",
            "preview stays blank for ciphertext content"
        );
        assert!(meta.protected);
    }

    #[test]
    fn encrypting_purges_the_plaintext_revision_history() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "a", "<p>v1</p>");
        seed(&s, "b", "<p>v1</p>");
        crate::revisions::add_revision(&s.conn, "a", "<p>v1</p>", 50).unwrap();
        crate::revisions::add_revision(&s.conn, "b", "<p>v1</p>", 50).unwrap();
        assert_eq!(revision_count(&s, "a"), 1);

        encrypt_note_in_place(&s, "a", &dek).unwrap();

        assert_eq!(revision_count(&s, "a"), 0);
        assert_eq!(
            revision_count(&s, "b"),
            1,
            "untouched note keeps its history"
        );
    }

    #[test]
    fn encrypting_marks_the_row_dirty_for_push_when_syncing() {
        // I1: the freshly-sealed ciphertext + `protected = 1` must be pushed,
        // instead of the server retaining the pre-protection plaintext (and a
        // later resync clobbering local ciphertext back under the LWW guard).
        let s = syncing_store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>secret</p>");
        clear_dirty(&s);

        encrypt_note_in_place(&s, "n1", &dek).unwrap();

        let dirty = s.load_dirty_notes().unwrap();
        assert_eq!(dirty.len(), 1);
        assert_eq!(dirty[0].id, "n1");
        assert!(
            dirty[0].protected,
            "the pushed row must carry protected = 1"
        );
        assert!(!dirty[0].content.contains("secret"));
    }

    #[test]
    fn encrypting_leaves_a_local_context_clean() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>secret</p>");

        encrypt_note_in_place(&s, "n1", &dek).unwrap();

        assert!(
            s.load_dirty_notes().unwrap().is_empty(),
            "a non-syncing context has nothing to push"
        );
    }

    #[test]
    fn encrypting_a_missing_note_is_a_harmless_no_op() {
        let s = store();
        let dek = Dek::random();
        // Nothing to seal and no row to update: every statement matches zero
        // rows, so no phantom note is created and nothing is left half-done.
        encrypt_note_in_place(&s, "ghost", &dek).unwrap();
        assert!(s.load_notes().unwrap().is_empty());
        assert!(s.load_note_content("ghost").unwrap().is_none());
    }
}

#[cfg(test)]
mod backfill_tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn fills_in_empty_titles_after_unlock() {
        let s = store();
        let dek = Dek::random();
        let sealed = seal_content(&dek, "n1", "<p>Old Secret</p><p>body</p>");
        s.save_note(&note("n1", &sealed)).unwrap();
        s.set_note_protected("n1", true).unwrap();
        assert_eq!(title_of(&s, "n1"), "");

        backfill_protected_titles(&s, &dek);

        assert_eq!(title_of(&s, "n1"), "Old Secret");
        assert!(
            s.note_protected("n1").unwrap(),
            "protected flag is untouched"
        );
        assert_eq!(
            content_of(&s, "n1"),
            sealed,
            "content is read but never rewritten"
        );
    }

    #[test]
    fn marks_dirty_for_push_when_syncing() {
        let s = syncing_store();
        let dek = Dek::random();
        let sealed = seal_content(&dek, "n1", "<p>Old Secret</p>");
        s.save_note(&note("n1", &sealed)).unwrap();
        s.set_note_protected("n1", true).unwrap();
        clear_dirty(&s);

        backfill_protected_titles(&s, &dek);

        let dirty = s.load_dirty_notes().unwrap();
        assert_eq!(dirty.len(), 1);
        assert_eq!(dirty[0].id, "n1");
        assert!(dirty[0].protected, "still protected");
        assert_eq!(dirty[0].content, sealed, "content is never rewritten");
    }

    #[test]
    fn skips_notes_that_fail_to_decrypt() {
        let s = store();
        let right = Dek::random();
        let wrong = Dek::random();
        s.save_note(&note(
            "n1",
            &seal_content(&right, "n1", "<p>Unreadable</p>"),
        ))
        .unwrap();
        s.set_note_protected("n1", true).unwrap();

        // Unlocking with the WRONG dek must not panic or abort — just skip.
        backfill_protected_titles(&s, &wrong);

        assert_eq!(title_of(&s, "n1"), "", "skipped note keeps its empty title");
        assert!(s.note_protected("n1").unwrap());
    }

    #[test]
    fn leaves_notes_that_already_have_a_title_alone() {
        let s = store();
        let dek = Dek::random();
        s.save_note(&note("n1", &seal_content(&dek, "n1", "<p>Real Body</p>")))
            .unwrap();
        s.set_note_protected("n1", true).unwrap();
        s.set_title("n1", "Handpicked").unwrap();

        backfill_protected_titles(&s, &dek);

        assert_eq!(title_of(&s, "n1"), "Handpicked");
    }

    #[test]
    fn ignores_unprotected_notes_entirely() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>plain</p>"); // `save_note` leaves `title` empty

        backfill_protected_titles(&s, &dek);

        assert_eq!(
            title_of(&s, "n1"),
            "",
            "only protected rows are candidates for the backfill"
        );
        assert_eq!(content_of(&s, "n1"), "<p>plain</p>");
    }

    #[test]
    fn is_a_no_op_on_an_empty_database() {
        let s = store();
        backfill_protected_titles(&s, &Dek::random());
        assert!(s.load_notes().unwrap().is_empty());
    }
}

#[cfg(test)]
mod note_read_tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn missing_note_reads_as_empty_string() {
        let s = store();
        assert_eq!(load_note_content(&s, None, "ghost").unwrap(), "");
    }

    #[test]
    fn plaintext_note_reads_back_verbatim_without_a_dek() {
        let s = store();
        seed(&s, "n1", "<p>hello</p>");
        assert_eq!(load_note_content(&s, None, "n1").unwrap(), "<p>hello</p>");
    }

    #[test]
    fn protected_note_is_decrypted_when_the_vault_is_unlocked() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>secret</p>");
        encrypt_note_in_place(&s, "n1", &dek).unwrap();

        assert_eq!(
            load_note_content(&s, Some(&dek), "n1").unwrap(),
            "<p>secret</p>"
        );
    }

    #[test]
    fn protected_note_is_refused_while_the_vault_is_locked() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>secret</p>");
        encrypt_note_in_place(&s, "n1", &dek).unwrap();

        assert_eq!(
            load_note_content(&s, None, "n1").unwrap_err(),
            "vault locked"
        );
    }

    #[test]
    fn protected_note_under_a_foreign_dek_errors_instead_of_leaking_ciphertext() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>secret</p>");
        encrypt_note_in_place(&s, "n1", &dek).unwrap();

        let err = load_note_content(&s, Some(&Dek::random()), "n1").unwrap_err();
        assert!(!err.contains("secret"));
        assert!(!err.is_empty());
    }

    #[test]
    fn a_note_in_a_locked_folder_that_is_still_plaintext_reads_back_plainly() {
        // `protected` — not the folder flag — is what gates decryption on read,
        // matching the "content is ciphertext iff protected = 1" invariant.
        let s = store();
        folder(&s, "f", None);
        seed_in(&s, "n1", "<p>plain</p>", "f");
        s.set_folder_locked("f", true).unwrap();

        assert_eq!(load_note_content(&s, None, "n1").unwrap(), "<p>plain</p>");
    }
}

#[cfg(test)]
mod search_tests {
    use super::test_support::*;
    use super::*;

    fn ids(hits: &[SearchHit]) -> Vec<String> {
        hits.iter().map(|h| h.note.id.clone()).collect()
    }

    #[test]
    fn finds_notes_by_body_text() {
        let s = store();
        seed(&s, "n1", "<p>Groceries</p><p>buy milk</p>");
        seed(&s, "n2", "<p>Taxes</p><p>file forms</p>");

        let hits = search_notes(&s, "milk", true).unwrap();
        assert_eq!(ids(&hits), vec!["n1"]);
        assert!(hits[0].snippet.contains("milk"));
    }

    #[test]
    fn empty_query_returns_nothing() {
        let s = store();
        seed(&s, "n1", "<p>anything</p>");
        assert!(search_notes(&s, "", true).unwrap().is_empty());
        assert!(search_notes(&s, "   ", true).unwrap().is_empty());
    }

    #[test]
    fn protected_notes_are_excluded_while_the_vault_is_locked() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>Password Vault</p><p>hunter2</p>");
        seed(&s, "n2", "<p>Shopping</p><p>hunter2 is a joke</p>");
        encrypt_note_in_place(&s, "n1", &dek).unwrap();

        let locked = search_notes(&s, "hunter2", false).unwrap();
        assert_eq!(ids(&locked), vec!["n2"], "ciphertext rows are dropped");

        let unlocked = search_notes(&s, "hunter2", true).unwrap();
        assert_eq!(
            unlocked.len(),
            1,
            "an unlocked vault still can't match sealed body text, but the row is a candidate"
        );
    }

    #[test]
    fn a_protected_notes_body_never_matches_in_either_lock_state() {
        // Locked, the ciphertext row is filtered out; unlocked it becomes a
        // candidate again, but `preview` is blanked for ciphertext and the
        // stored body is base64, so neither the title nor the body text can
        // produce a match. Full-text search over sealed notes is out of scope.
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>Bank Codes</p><p>secret body</p>");
        encrypt_note_in_place(&s, "n1", &dek).unwrap();

        assert!(search_notes(&s, "Bank", false).unwrap().is_empty());
        assert!(search_notes(&s, "Bank", true).unwrap().is_empty());
        assert!(search_notes(&s, "secret body", true).unwrap().is_empty());
    }

    #[test]
    fn results_are_capped_at_fifty() {
        let s = store();
        for i in 0..60 {
            seed(&s, &format!("n{i}"), "<p>needle</p>");
        }
        assert_eq!(search_notes(&s, "needle", true).unwrap().len(), 50);
    }

    #[test]
    fn search_all_contexts_spans_every_registry_entry() {
        let dir = tempfile::tempdir().unwrap();
        let mut ctxs = Vec::new();
        for (i, name) in ["one", "two"].iter().enumerate() {
            let path = dir.path().join(format!("{name}.db"));
            let s = Store::open(&path).unwrap();
            crate::migrate::run_migrations(&s.conn).unwrap();
            seed(&s, &format!("n{i}"), "<p>shared needle</p>");
            ctxs.push(crate::aggregate::Ctx {
                id: name.to_string(),
                label: name.to_string(),
                kind: "local".into(),
                path: path.to_string_lossy().into_owned(),
            });
        }

        let hits = search_all_contexts(&ctxs, "needle", true);
        assert_eq!(hits.len(), 2);
        let mut labels: Vec<&str> = hits.iter().map(|h| h.context_label.as_str()).collect();
        labels.sort();
        assert_eq!(labels, vec!["one", "two"]);
    }

    #[test]
    fn search_all_contexts_honors_the_vault_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.db");
        let dek = Dek::random();
        {
            let s = Store::open(&path).unwrap();
            crate::migrate::run_migrations(&s.conn).unwrap();
            seed(&s, "n1", "<p>needle</p>");
            encrypt_note_in_place(&s, "n1", &dek).unwrap();
        }
        let ctxs = vec![crate::aggregate::Ctx {
            id: "c".into(),
            label: "c".into(),
            kind: "local".into(),
            path: path.to_string_lossy().into_owned(),
        }];

        assert!(
            search_all_contexts(&ctxs, "needle", false).is_empty(),
            "locked vault excludes the ciphertext row"
        );
    }
}

#[cfg(test)]
mod save_tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn unprotected_save_stores_plaintext_and_derives_the_title() {
        let s = store();

        save_note(&s, None, &note("n1", "<p>My Title</p><p>body text</p>")).unwrap();

        assert_eq!(content_of(&s, "n1"), "<p>My Title</p><p>body text</p>");
        let meta = &s.load_notes_meta().unwrap()[0];
        assert_eq!(meta.title, "My Title");
        assert_eq!(meta.preview, "My Title");
        assert!(!meta.protected);
    }

    #[test]
    fn unprotected_save_records_a_revision() {
        let s = store();
        save_note(&s, None, &note("n1", "<p>v1</p>")).unwrap();
        save_note(&s, None, &note("n1", "<p>v2</p>")).unwrap();

        assert_eq!(revision_count(&s, "n1"), 2);
    }

    #[test]
    fn unprotected_save_honors_the_revision_limit_setting() {
        let s = store();
        crate::settings::set_setting(&s.conn, "revisionLimit", "2").unwrap();
        for i in 0..5 {
            save_note(&s, None, &note("n1", &format!("<p>v{i}</p>"))).unwrap();
        }
        assert_eq!(revision_count(&s, "n1"), 2);
    }

    #[test]
    fn protected_save_stores_ciphertext_and_keeps_a_plaintext_title() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>x</p>");
        s.set_note_protected("n1", true).unwrap();

        save_note(&s, Some(&dek), &note("n1", "<p>Very Secret</p><p>body</p>")).unwrap();

        let stored = content_of(&s, "n1");
        assert!(!stored.contains("Very Secret"));
        assert!(!stored.contains("body"));
        assert_eq!(
            open_content(&dek, "n1", &stored).unwrap(),
            "<p>Very Secret</p><p>body</p>"
        );
        assert_eq!(title_of(&s, "n1"), "Very Secret");
        assert!(s.note_protected("n1").unwrap(), "protected stays set");
    }

    #[test]
    fn protected_save_never_leaves_a_plaintext_revision_behind() {
        let s = store();
        let dek = Dek::random();
        // Plaintext history exists from before the note was protected.
        save_note(&s, None, &note("n1", "<p>old plaintext</p>")).unwrap();
        assert_eq!(revision_count(&s, "n1"), 1);
        s.set_note_protected("n1", true).unwrap();

        save_note(&s, Some(&dek), &note("n1", "<p>new secret</p>")).unwrap();

        assert_eq!(
            revision_count(&s, "n1"),
            0,
            "pre-transition plaintext revisions are purged and no new one is added"
        );
    }

    #[test]
    fn protected_save_is_refused_while_the_vault_is_locked() {
        let s = store();
        seed(&s, "n1", "<p>original</p>");
        s.set_note_protected("n1", true).unwrap();

        let err = save_note(&s, None, &note("n1", "<p>would-be plaintext</p>")).unwrap_err();
        assert_eq!(err, "vault locked");
        assert_eq!(
            content_of(&s, "n1"),
            "<p>original</p>",
            "nothing is written when the vault is locked"
        );
    }

    #[test]
    fn a_note_in_a_locked_folder_is_saved_as_ciphertext() {
        // Effective protection comes from the folder, not the note's own flag.
        let s = store();
        let dek = Dek::random();
        folder(&s, "f", None);
        s.set_folder_locked("f", true).unwrap();
        let mut n = note("n1", "<p>Folder Secret</p>");
        n.folder_id = Some("f".into());
        s.save_note(&n).unwrap();

        save_note(&s, Some(&dek), &n).unwrap();

        let stored = content_of(&s, "n1");
        assert!(!stored.contains("Folder Secret"));
        assert!(
            s.note_protected("n1").unwrap(),
            "the physical flag is brought in line with the folder's intent"
        );
        assert_eq!(
            open_content(&dek, "n1", &stored).unwrap(),
            "<p>Folder Secret</p>"
        );
    }

    #[test]
    fn a_note_in_a_locked_folder_is_refused_while_the_vault_is_locked() {
        let s = store();
        folder(&s, "f", None);
        s.set_folder_locked("f", true).unwrap();
        let mut n = note("n1", "<p>plain</p>");
        n.folder_id = Some("f".into());
        s.save_note(&n).unwrap();

        assert_eq!(save_note(&s, None, &n).unwrap_err(), "vault locked");
        assert_eq!(content_of(&s, "n1"), "<p>plain</p>");
    }

    #[test]
    fn saving_a_brand_new_note_inserts_it() {
        let s = store();
        save_note(&s, None, &note("fresh", "<p>Brand New</p>")).unwrap();
        let notes = s.load_notes().unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id, "fresh");
    }
}

#[cfg(test)]
mod delete_tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn a_syncing_context_tombstones_instead_of_deleting() {
        let s = syncing_store();
        seed(&s, "n1", "<p>x</p>");

        delete_note(&s, "n1", 1_000).unwrap();

        assert!(
            s.load_notes().unwrap().is_empty(),
            "gone from the active list"
        );
        let dirty = s.load_dirty_notes().unwrap();
        assert_eq!(dirty.len(), 1);
        assert!(
            dirty[0].deleted_at.is_some(),
            "tombstone is queued for push"
        );
    }

    #[test]
    fn a_local_context_trashes_by_default() {
        let s = store();
        seed(&s, "n1", "<p>x</p>");

        delete_note(&s, "n1", 1_234).unwrap();

        assert!(s.load_notes().unwrap().is_empty());
        let trashed = s.load_trashed_meta().unwrap();
        assert_eq!(trashed.len(), 1);
        assert_eq!(trashed[0].deleted_at, Some(1_234));
    }

    #[test]
    fn trash_disabled_deletes_outright() {
        let s = store();
        crate::settings::set_setting(&s.conn, "trashEnabled", "false").unwrap();
        seed(&s, "n1", "<p>x</p>");
        crate::revisions::add_revision(&s.conn, "n1", "<p>x</p>", 50).unwrap();

        delete_note(&s, "n1", 1_234).unwrap();

        assert!(s.load_notes().unwrap().is_empty());
        assert!(s.load_trashed_meta().unwrap().is_empty());
        assert_eq!(revision_count(&s, "n1"), 0, "revisions go with the note");
    }

    #[test]
    fn trash_enabled_true_is_explicit_opt_in_to_the_default() {
        let s = store();
        crate::settings::set_setting(&s.conn, "trashEnabled", "true").unwrap();
        seed(&s, "n1", "<p>x</p>");

        delete_note(&s, "n1", 7).unwrap();

        assert_eq!(s.load_trashed_meta().unwrap().len(), 1);
    }

    #[test]
    fn deleting_a_missing_note_is_a_no_op() {
        let s = store();
        delete_note(&s, "ghost", 1).unwrap();
        assert!(s.load_notes().unwrap().is_empty());
    }
}

#[cfg(test)]
mod stats_tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn counts_notes_and_tasks() {
        let s = store();
        seed(&s, "n1", "<ul><li>a</li></ul><p>some words here</p>");
        seed(&s, "n2", "<p>another note</p>");

        let stats = note_stats(&s).unwrap();
        assert_eq!(stats.notes, 2);
        assert!(stats.words > 0);
    }

    #[test]
    fn an_empty_store_reports_zero_notes() {
        let s = store();
        assert_eq!(note_stats(&s).unwrap().notes, 0);
    }

    #[test]
    fn trashed_notes_are_not_counted() {
        let s = store();
        seed(&s, "n1", "<p>kept</p>");
        seed(&s, "n2", "<p>trashed</p>");
        s.trash_note("n2", 1).unwrap();

        assert_eq!(note_stats(&s).unwrap().notes, 1);
    }
}

#[cfg(test)]
mod folder_tests {
    use super::test_support::*;
    use super::*;

    fn folders(s: &Store) -> Vec<crate::folders::Folder> {
        crate::folders::load_folders(&s.conn).unwrap()
    }

    fn one(s: &Store, id: &str) -> crate::folders::Folder {
        folders(s).into_iter().find(|f| f.id == id).unwrap()
    }

    fn dirty_folder_ids(s: &Store) -> Vec<String> {
        crate::folders::load_dirty_folders(&s.conn)
            .unwrap()
            .into_iter()
            .map(|f| f.id)
            .collect()
    }

    #[test]
    fn create_inserts_a_root_folder() {
        let s = store();
        folder_create(&s, "f1", "Work", None).unwrap();

        let all = folders(&s);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "Work");
        assert_eq!(all[0].parent_id, None);
    }

    #[test]
    fn create_nests_under_a_parent() {
        let s = store();
        folder_create(&s, "top", "Top", None).unwrap();
        folder_create(&s, "sub", "Sub", Some("top")).unwrap();

        assert_eq!(one(&s, "sub").parent_id.as_deref(), Some("top"));
    }

    #[test]
    fn create_marks_the_folder_dirty_only_when_syncing() {
        let local = store();
        folder_create(&local, "f1", "Work", None).unwrap();
        assert!(dirty_folder_ids(&local).is_empty());

        let remote = syncing_store();
        folder_create(&remote, "f1", "Work", None).unwrap();
        assert_eq!(dirty_folder_ids(&remote), vec!["f1"]);
    }

    #[test]
    fn rename_changes_the_name_and_dirties_when_syncing() {
        let s = syncing_store();
        folder_create(&s, "f1", "Old", None).unwrap();
        crate::folders::clear_folder_dirty(&s.conn, &[("f1".into(), one(&s, "f1").updated_at)])
            .unwrap();

        folder_rename(&s, "f1", "New").unwrap();

        assert_eq!(one(&s, "f1").name, "New");
        assert_eq!(dirty_folder_ids(&s), vec!["f1"]);
    }

    #[test]
    fn move_reparents_and_can_return_to_root() {
        let s = store();
        folder_create(&s, "top", "Top", None).unwrap();
        folder_create(&s, "sub", "Sub", None).unwrap();

        folder_move(&s, "sub", Some("top")).unwrap();
        assert_eq!(one(&s, "sub").parent_id.as_deref(), Some("top"));

        folder_move(&s, "sub", None).unwrap();
        assert_eq!(one(&s, "sub").parent_id, None);
    }

    #[test]
    fn delete_reparent_keeps_the_children() {
        let s = store();
        folder_create(&s, "top", "Top", None).unwrap();
        folder_create(&s, "sub", "Sub", Some("top")).unwrap();

        folder_delete(&s, "top", "reparent").unwrap();

        let all = folders(&s);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "sub");
        assert_eq!(all[0].parent_id, None, "children move up to the root");
    }

    #[test]
    fn delete_recursive_removes_the_whole_subtree() {
        let s = store();
        folder_create(&s, "top", "Top", None).unwrap();
        folder_create(&s, "sub", "Sub", Some("top")).unwrap();

        folder_delete(&s, "top", "recursive").unwrap();

        assert!(folders(&s).is_empty());
    }

    #[test]
    fn an_unknown_delete_mode_falls_back_to_reparent() {
        let s = store();
        folder_create(&s, "top", "Top", None).unwrap();
        folder_create(&s, "sub", "Sub", Some("top")).unwrap();

        folder_delete(&s, "top", "who-knows").unwrap();

        assert_eq!(folders(&s).len(), 1, "same as reparent");
    }

    #[test]
    fn a_syncing_delete_tombstones_so_it_propagates() {
        let s = syncing_store();
        folder_create(&s, "f1", "Work", None).unwrap();

        folder_delete(&s, "f1", "reparent").unwrap();

        assert!(folders(&s).is_empty(), "hidden from the active list");
        let dirty = crate::folders::load_dirty_folders(&s.conn).unwrap();
        assert_eq!(dirty.len(), 1);
        assert!(dirty[0].deleted_at.is_some());
    }

    #[test]
    fn reorder_assigns_positions_in_the_given_order() {
        let s = store();
        for id in ["a", "b", "c"] {
            folder_create(&s, id, id, None).unwrap();
        }

        folders_reorder(&s, None, &["c".into(), "a".into(), "b".into()]).unwrap();

        let mut all = folders(&s);
        all.sort_by_key(|f| f.position);
        let order: Vec<&str> = all.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(order, vec!["c", "a", "b"]);
    }

    #[test]
    fn reorder_with_an_empty_list_changes_nothing() {
        let s = store();
        folder_create(&s, "a", "a", None).unwrap();
        folders_reorder(&s, None, &[]).unwrap();
        assert_eq!(folders(&s).len(), 1);
    }

    #[test]
    fn icon_color_and_sort_are_persisted() {
        let s = store();
        folder_create(&s, "f1", "Work", None).unwrap();

        folder_set_icon(&s, "f1", "star").unwrap();
        folder_set_color(&s, "f1", "#ff0000").unwrap();
        folder_set_sort(&s, "f1", "manual").unwrap();

        let f = one(&s, "f1");
        assert_eq!(f.icon, "star");
        assert_eq!(f.color, "#ff0000");
        assert_eq!(f.sort, "manual");
    }

    #[test]
    fn icon_color_and_sort_dirty_the_row_only_when_syncing() {
        let local = store();
        folder_create(&local, "f1", "Work", None).unwrap();
        folder_set_icon(&local, "f1", "star").unwrap();
        folder_set_color(&local, "f1", "#abc").unwrap();
        folder_set_sort(&local, "f1", "name").unwrap();
        assert!(dirty_folder_ids(&local).is_empty());

        let remote = syncing_store();
        folder_create(&remote, "f2", "Work", None).unwrap();
        crate::folders::clear_folder_dirty(
            &remote.conn,
            &[("f2".into(), one(&remote, "f2").updated_at)],
        )
        .unwrap();
        folder_set_icon(&remote, "f2", "star").unwrap();
        assert_eq!(dirty_folder_ids(&remote), vec!["f2"]);
    }

    #[test]
    fn mutating_an_unknown_folder_is_a_silent_no_op() {
        let s = store();
        folder_rename(&s, "ghost", "X").unwrap();
        folder_set_icon(&s, "ghost", "star").unwrap();
        folder_set_color(&s, "ghost", "#fff").unwrap();
        folder_set_sort(&s, "ghost", "name").unwrap();
        folder_move(&s, "ghost", None).unwrap();
        assert!(folders(&s).is_empty());
    }
}

#[cfg(test)]
mod reconcile_move_tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn plain_move_into_an_unlocked_folder_leaves_the_note_plaintext() {
        let s = store();
        folder(&s, "f", None);
        seed(&s, "n1", "<p>plain</p>");

        reconcile_folder_move(&s, "n1", Some("f"), None).unwrap();

        assert_eq!(s.load_notes().unwrap()[0].folder_id.as_deref(), Some("f"));
        assert!(!s.note_protected("n1").unwrap());
        assert_eq!(content_of(&s, "n1"), "<p>plain</p>");
    }

    #[test]
    fn move_back_to_the_root_is_allowed() {
        let s = store();
        folder(&s, "f", None);
        seed_in(&s, "n1", "<p>plain</p>", "f");

        reconcile_folder_move(&s, "n1", None, None).unwrap();

        assert_eq!(s.load_notes().unwrap()[0].folder_id, None);
    }

    #[test]
    fn moving_a_plaintext_note_into_a_locked_folder_encrypts_it() {
        let s = store();
        folder(&s, "locked-folder", None);
        s.set_folder_locked("locked-folder", true).unwrap();
        let dek = Dek::random();
        seed(&s, "n1", "<p>very secret</p>");
        crate::revisions::add_revision(&s.conn, "n1", "<p>very secret</p>", 50).unwrap();
        assert_eq!(revision_count(&s, "n1"), 1);

        reconcile_folder_move(&s, "n1", Some("locked-folder"), Some(&dek)).unwrap();

        assert_eq!(
            s.load_notes().unwrap()[0].folder_id.as_deref(),
            Some("locked-folder")
        );
        assert!(s.note_protected("n1").unwrap());
        let stored = content_of(&s, "n1");
        assert!(!stored.contains("very secret"));
        assert_eq!(
            open_content(&dek, "n1", &stored).unwrap(),
            "<p>very secret</p>"
        );
        assert_eq!(revision_count(&s, "n1"), 0);
    }

    #[test]
    fn moving_into_a_folder_whose_ancestor_is_locked_also_encrypts() {
        let s = store();
        folder(&s, "top", None);
        folder(&s, "sub", Some("top"));
        s.set_folder_locked("top", true).unwrap();
        let dek = Dek::random();
        seed(&s, "n1", "<p>nested secret</p>");

        reconcile_folder_move(&s, "n1", Some("sub"), Some(&dek)).unwrap();

        assert!(s.note_protected("n1").unwrap());
        assert!(!content_of(&s, "n1").contains("nested secret"));
    }

    #[test]
    fn a_locked_vault_refuses_and_leaves_the_note_exactly_where_it_was() {
        let s = store();
        folder(&s, "locked-folder", None);
        s.set_folder_locked("locked-folder", true).unwrap();
        seed(&s, "n1", "<p>very secret</p>");
        crate::revisions::add_revision(&s.conn, "n1", "<p>very secret</p>", 50).unwrap();

        let err = reconcile_folder_move(&s, "n1", Some("locked-folder"), None).unwrap_err();
        assert_eq!(err, "vault locked");

        assert_eq!(s.load_notes().unwrap()[0].folder_id, None, "unmoved");
        assert!(!s.note_protected("n1").unwrap());
        assert_eq!(content_of(&s, "n1"), "<p>very secret</p>");
        assert_eq!(revision_count(&s, "n1"), 1, "revision untouched");
    }

    #[test]
    fn an_already_encrypted_note_moves_into_a_locked_folder_even_when_locked() {
        // Nothing to seal, so no DEK is needed — the note is already ciphertext.
        let s = store();
        folder(&s, "locked-folder", None);
        s.set_folder_locked("locked-folder", true).unwrap();
        let dek = Dek::random();
        seed(&s, "n1", "<p>secret</p>");
        encrypt_note_in_place(&s, "n1", &dek).unwrap();
        let before = content_of(&s, "n1");

        reconcile_folder_move(&s, "n1", Some("locked-folder"), None).unwrap();

        assert_eq!(
            s.load_notes().unwrap()[0].folder_id.as_deref(),
            Some("locked-folder")
        );
        assert_eq!(content_of(&s, "n1"), before, "no double encryption");
    }

    #[test]
    fn moving_an_encrypted_note_out_of_a_locked_folder_never_auto_decrypts_it() {
        let s = store();
        folder(&s, "locked-folder", None);
        s.set_folder_locked("locked-folder", true).unwrap();
        let dek = Dek::random();
        seed_in(&s, "n1", "<p>secret</p>", "locked-folder");
        encrypt_note_in_place(&s, "n1", &dek).unwrap();
        let sealed = content_of(&s, "n1");

        reconcile_folder_move(&s, "n1", None, Some(&dek)).unwrap();

        assert_eq!(s.load_notes().unwrap()[0].folder_id, None);
        assert!(s.note_protected("n1").unwrap(), "still protected");
        assert_eq!(content_of(&s, "n1"), sealed, "still ciphertext");
    }

    #[test]
    fn moving_a_missing_note_is_an_error_and_creates_nothing() {
        let s = store();
        assert!(reconcile_folder_move(&s, "ghost", None, None).is_err());
        assert!(s.load_notes().unwrap().is_empty());
    }
}

#[cfg(test)]
mod reconcile_reorder_tests {
    use super::test_support::*;
    use super::*;

    fn positions(s: &Store) -> Vec<(String, i64)> {
        let mut v: Vec<(String, i64)> = s
            .load_notes()
            .unwrap()
            .into_iter()
            .map(|n| (n.id, n.position))
            .collect();
        v.sort_by_key(|(_, p)| *p);
        v
    }

    #[test]
    fn plain_reorder_assigns_positions_in_order() {
        let s = store();
        for id in ["a", "b", "c"] {
            seed(&s, id, "<p>x</p>");
        }

        reconcile_reorder(&s, None, &["c".into(), "a".into(), "b".into()], None).unwrap();

        let order: Vec<String> = positions(&s).into_iter().map(|(id, _)| id).collect();
        assert_eq!(order, vec!["c", "a", "b"]);
    }

    #[test]
    fn reorder_moves_notes_into_the_destination_folder() {
        let s = store();
        folder(&s, "f", None);
        seed(&s, "a", "<p>x</p>");

        reconcile_reorder(&s, Some("f"), &["a".into()], None).unwrap();

        assert_eq!(s.load_notes().unwrap()[0].folder_id.as_deref(), Some("f"));
    }

    #[test]
    fn dropping_a_plaintext_note_into_a_locked_folder_encrypts_it() {
        let s = store();
        folder(&s, "locked-folder", None);
        s.set_folder_locked("locked-folder", true).unwrap();
        let dek = Dek::random();
        seed(&s, "n1", "<p>very secret</p>");
        crate::revisions::add_revision(&s.conn, "n1", "<p>very secret</p>", 50).unwrap();

        reconcile_reorder(&s, Some("locked-folder"), &["n1".into()], Some(&dek)).unwrap();

        assert_eq!(
            s.load_notes().unwrap()[0].folder_id.as_deref(),
            Some("locked-folder")
        );
        assert!(s.note_protected("n1").unwrap());
        let stored = content_of(&s, "n1");
        assert!(!stored.contains("very secret"));
        assert_eq!(
            open_content(&dek, "n1", &stored).unwrap(),
            "<p>very secret</p>"
        );
        assert_eq!(revision_count(&s, "n1"), 0);
    }

    #[test]
    fn a_locked_vault_refuses_the_whole_batch_without_touching_a_single_row() {
        let s = store();
        folder(&s, "locked-folder", None);
        s.set_folder_locked("locked-folder", true).unwrap();
        seed(&s, "plain", "<p>very secret</p>");
        seed(&s, "already", "<p>other</p>");
        encrypt_note_in_place(&s, "already", &Dek::random()).unwrap();

        let err = reconcile_reorder(
            &s,
            Some("locked-folder"),
            &["already".into(), "plain".into()],
            None,
        )
        .unwrap_err();
        assert_eq!(err, "vault locked");

        // NOTHING moved — not even the already-encrypted note that would have
        // been safe on its own.
        for n in s.load_notes().unwrap() {
            assert_eq!(n.folder_id, None, "{} must not have moved", n.id);
        }
        assert_eq!(content_of(&s, "plain"), "<p>very secret</p>");
    }

    #[test]
    fn only_the_plaintext_notes_of_a_batch_get_encrypted() {
        let s = store();
        folder(&s, "locked-folder", None);
        s.set_folder_locked("locked-folder", true).unwrap();
        let dek = Dek::random();
        seed(&s, "plain", "<p>fresh secret</p>");
        seed(&s, "already", "<p>old secret</p>");
        encrypt_note_in_place(&s, "already", &dek).unwrap();
        let already_sealed = content_of(&s, "already");

        reconcile_reorder(
            &s,
            Some("locked-folder"),
            &["already".into(), "plain".into()],
            Some(&dek),
        )
        .unwrap();

        assert_eq!(
            content_of(&s, "already"),
            already_sealed,
            "an already-sealed note is never re-sealed"
        );
        assert!(s.note_protected("plain").unwrap());
        assert_eq!(
            open_content(&dek, "plain", &content_of(&s, "plain")).unwrap(),
            "<p>fresh secret</p>"
        );
    }

    #[test]
    fn an_all_encrypted_batch_into_a_locked_folder_needs_no_dek() {
        let s = store();
        folder(&s, "locked-folder", None);
        s.set_folder_locked("locked-folder", true).unwrap();
        seed(&s, "a", "<p>x</p>");
        encrypt_note_in_place(&s, "a", &Dek::random()).unwrap();

        reconcile_reorder(&s, Some("locked-folder"), &["a".into()], None).unwrap();

        assert_eq!(
            s.load_notes().unwrap()[0].folder_id.as_deref(),
            Some("locked-folder")
        );
    }

    #[test]
    fn a_stale_id_in_the_drag_payload_is_skipped_not_fatal() {
        let s = store();
        folder(&s, "locked-folder", None);
        s.set_folder_locked("locked-folder", true).unwrap();
        let dek = Dek::random();
        seed(&s, "real", "<p>secret</p>");

        reconcile_reorder(
            &s,
            Some("locked-folder"),
            &["ghost".into(), "real".into()],
            Some(&dek),
        )
        .unwrap();

        assert_eq!(s.load_notes().unwrap().len(), 1);
        assert!(s.note_protected("real").unwrap());
    }

    #[test]
    fn a_stale_id_alone_does_not_trigger_the_locked_vault_refusal() {
        // Only *existing plaintext* rows require a DEK; a missing id must not
        // make an otherwise-safe reorder fail.
        let s = store();
        folder(&s, "locked-folder", None);
        s.set_folder_locked("locked-folder", true).unwrap();

        reconcile_reorder(&s, Some("locked-folder"), &["ghost".into()], None).unwrap();
    }

    #[test]
    fn an_empty_id_list_into_a_locked_folder_is_a_no_op() {
        let s = store();
        folder(&s, "locked-folder", None);
        s.set_folder_locked("locked-folder", true).unwrap();

        reconcile_reorder(&s, Some("locked-folder"), &[], None).unwrap();

        assert!(s.load_notes().unwrap().is_empty());
    }
}

#[cfg(test)]
mod folder_guard_tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn creating_a_duplicate_folder_id_is_rejected() {
        let s = store();
        folder_create(&s, "f1", "Work", None).unwrap();
        assert!(folder_create(&s, "f1", "Other", None).is_err());
    }

    #[test]
    fn a_folder_cannot_be_moved_into_itself_or_a_descendant() {
        let s = store();
        folder_create(&s, "top", "Top", None).unwrap();
        folder_create(&s, "sub", "Sub", Some("top")).unwrap();

        assert!(folder_move(&s, "top", Some("top")).is_err());
        assert!(folder_move(&s, "top", Some("sub")).is_err());
        // The rejected move left the tree untouched.
        let all = crate::folders::load_folders(&s.conn).unwrap();
        assert_eq!(all.iter().find(|f| f.id == "top").unwrap().parent_id, None);
    }

    #[test]
    fn reorder_rejects_making_a_folder_its_own_parent() {
        let s = store();
        folder_create(&s, "top", "Top", None).unwrap();
        folder_create(&s, "sub", "Sub", Some("top")).unwrap();

        assert!(folders_reorder(&s, Some("top"), &["top".into()]).is_err());
        assert!(folders_reorder(&s, Some("sub"), &["top".into()]).is_err());
    }
}

#[cfg(test)]
mod vault_record_tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn loading_without_a_vault_reports_not_set_up() {
        let s = store();
        assert_eq!(err_of(load_vault_record(&s)), "vault: not set up");
    }

    #[test]
    fn a_corrupt_record_is_rejected_rather_than_panicking() {
        let s = store();
        s.set_vault_record("{not json").unwrap();
        let err = err_of(load_vault_record(&s));
        assert!(err.contains("corrupt"), "unexpected error: {err}");
    }

    #[test]
    fn a_persisted_record_round_trips() {
        let s = store();
        let (_groups, _dek) = vault_setup(&s, "hunter2").unwrap();
        let rec = load_vault_record(&s).unwrap();
        assert!(!rec.dek_wrapped_pass.is_empty());
        assert!(!rec.dek_wrapped_recovery.is_empty());
    }
}

#[cfg(test)]
mod vault_setup_tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn setup_persists_a_record_and_returns_the_recovery_groups() {
        let s = store();

        let (groups, dek) = vault_setup(&s, "correct horse").unwrap();

        assert!(groups.len() > 1, "recovery key is shown in groups");
        assert!(groups.iter().all(|g| !g.is_empty()));
        assert!(s.vault_record().unwrap().is_some());
        // The returned DEK is the live one: it opens content sealed with it.
        let sealed = seal_content(&dek, "n1", "<p>x</p>");
        assert_eq!(open_content(&dek, "n1", &sealed).unwrap(), "<p>x</p>");
    }

    #[test]
    fn setup_refuses_to_clobber_an_existing_vault() {
        let s = store();
        vault_setup(&s, "first").unwrap();
        let before = s.vault_record().unwrap().unwrap();

        let err = err_of(vault_setup(&s, "second"));

        assert_eq!(err, "vault: a vault already exists");
        assert_eq!(
            s.vault_record().unwrap().unwrap(),
            before,
            "the stored record must not be overwritten — the old DEK would be orphaned"
        );
    }

    #[test]
    fn a_note_encrypted_under_the_first_vault_still_opens_after_a_refused_setup() {
        let s = store();
        let (_g, dek) = vault_setup(&s, "first").unwrap();
        seed(&s, "n1", "<p>irreplaceable</p>");
        encrypt_note_in_place(&s, "n1", &dek).unwrap();

        assert!(vault_setup(&s, "second").is_err());

        let reloaded = vault_unlock_passphrase(&record(&s), "first").unwrap();
        assert_eq!(
            open_content(&reloaded, "n1", &content_of(&s, "n1")).unwrap(),
            "<p>irreplaceable</p>"
        );
    }
}

#[cfg(test)]
mod vault_unlock_tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn the_right_passphrase_returns_a_usable_dek() {
        let s = store();
        let (_g, dek) = vault_setup(&s, "hunter2").unwrap();
        let sealed = seal_content(&dek, "n1", "<p>secret</p>");

        let unlocked = vault_unlock_passphrase(&record(&s), "hunter2").unwrap();

        assert_eq!(
            open_content(&unlocked, "n1", &sealed).unwrap(),
            "<p>secret</p>"
        );
    }

    #[test]
    fn a_wrong_passphrase_is_rejected_without_leaking_anything() {
        let s = store();
        vault_setup(&s, "hunter2").unwrap();

        let err = err_of(vault_unlock_passphrase(&record(&s), "hunter3"));
        assert!(!err.is_empty());
        assert!(
            !err.contains("hunter"),
            "the attempt must not be echoed back"
        );
    }

    #[test]
    fn unlocking_without_a_vault_reports_not_set_up() {
        // Both unlock commands load the record before deriving, so this is the
        // error the frontend sees when no vault exists.
        let s = store();
        assert_eq!(err_of(load_vault_record(&s)), "vault: not set up");
    }

    #[test]
    fn the_recovery_key_unlocks_the_same_dek() {
        let s = store();
        let (groups, dek) = vault_setup(&s, "hunter2").unwrap();
        let recovery = groups.join("-");
        let sealed = seal_content(&dek, "n1", "<p>secret</p>");

        let unlocked = vault_unlock_recovery(&record(&s), &recovery).unwrap();

        assert_eq!(
            open_content(&unlocked, "n1", &sealed).unwrap(),
            "<p>secret</p>"
        );
    }

    #[test]
    fn recovery_input_formatting_is_normalized() {
        let s = store();
        let (groups, dek) = vault_setup(&s, "hunter2").unwrap();
        let sealed = seal_content(&dek, "n1", "<p>secret</p>");
        // The user types it lowercase with spaces instead of dashes.
        let typed = groups.join(" ").to_lowercase();

        let unlocked = vault_unlock_recovery(&record(&s), &typed).unwrap();

        assert_eq!(
            open_content(&unlocked, "n1", &sealed).unwrap(),
            "<p>secret</p>"
        );
    }

    #[test]
    fn a_wrong_recovery_key_is_rejected() {
        let s = store();
        vault_setup(&s, "hunter2").unwrap();
        assert!(vault_unlock_recovery(&record(&s), "AAAA-BBBB-CCCC-DDDD").is_err());
    }
}

#[cfg(test)]
mod vault_change_passphrase_tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn the_new_passphrase_works_and_the_old_one_stops_working() {
        let s = store();
        vault_setup(&s, "old").unwrap();

        vault_change_passphrase(&s, "old", "new").unwrap();

        assert!(vault_unlock_passphrase(&record(&s), "old").is_err());
        assert!(vault_unlock_passphrase(&record(&s), "new").is_ok());
    }

    #[test]
    fn the_dek_is_unchanged_so_existing_ciphertext_still_opens() {
        let s = store();
        let (_g, dek) = vault_setup(&s, "old").unwrap();
        seed(&s, "n1", "<p>keepsake</p>");
        encrypt_note_in_place(&s, "n1", &dek).unwrap();

        let rewrapped = vault_change_passphrase(&s, "old", "new").unwrap();

        assert_eq!(
            open_content(&rewrapped, "n1", &content_of(&s, "n1")).unwrap(),
            "<p>keepsake</p>"
        );
    }

    #[test]
    fn the_recovery_key_keeps_working_after_a_passphrase_change() {
        let s = store();
        let (groups, _dek) = vault_setup(&s, "old").unwrap();
        let recovery = groups.join("-");

        vault_change_passphrase(&s, "old", "new").unwrap();

        assert!(vault_unlock_recovery(&record(&s), &recovery).is_ok());
    }

    #[test]
    fn a_wrong_current_passphrase_leaves_the_record_untouched() {
        let s = store();
        vault_setup(&s, "old").unwrap();
        let before = s.vault_record().unwrap().unwrap();

        assert!(vault_change_passphrase(&s, "wrong", "new").is_err());

        assert_eq!(s.vault_record().unwrap().unwrap(), before);
        assert!(vault_unlock_passphrase(&record(&s), "old").is_ok());
    }

    #[test]
    fn changing_without_a_vault_reports_not_set_up() {
        let s = store();
        assert_eq!(
            err_of(vault_change_passphrase(&s, "a", "b")),
            "vault: not set up"
        );
    }
}

#[cfg(test)]
mod note_protection_tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn protecting_seals_the_content_and_purges_revisions() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>Diary</p><p>dear diary</p>");
        crate::revisions::add_revision(&s.conn, "n1", "<p>Diary</p>", 50).unwrap();

        set_note_protected(&s, &dek, "n1", true).unwrap();

        assert!(s.note_protected("n1").unwrap());
        assert!(!content_of(&s, "n1").contains("dear diary"));
        assert_eq!(revision_count(&s, "n1"), 0);
        assert_eq!(title_of(&s, "n1"), "Diary", "title stays visible metadata");
    }

    #[test]
    fn protecting_an_already_protected_note_does_not_re_encrypt_it() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>secret</p>");
        set_note_protected(&s, &dek, "n1", true).unwrap();
        let sealed = content_of(&s, "n1");

        set_note_protected(&s, &dek, "n1", true).unwrap();

        assert_eq!(content_of(&s, "n1"), sealed, "no double encryption");
        assert_eq!(open_content(&dek, "n1", &sealed).unwrap(), "<p>secret</p>");
    }

    #[test]
    fn unprotecting_restores_the_plaintext() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>secret</p>");
        set_note_protected(&s, &dek, "n1", true).unwrap();

        set_note_protected(&s, &dek, "n1", false).unwrap();

        assert!(!s.note_protected("n1").unwrap());
        assert_eq!(content_of(&s, "n1"), "<p>secret</p>");
    }

    #[test]
    fn unprotecting_an_unprotected_note_is_a_no_op() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>plain</p>");

        set_note_protected(&s, &dek, "n1", false).unwrap();

        assert_eq!(content_of(&s, "n1"), "<p>plain</p>");
        assert!(!s.note_protected("n1").unwrap());
    }

    #[test]
    fn unprotecting_is_refused_while_the_note_sits_in_a_locked_folder() {
        let s = store();
        let dek = Dek::random();
        folder(&s, "f", None);
        seed_in(&s, "n1", "<p>secret</p>", "f");
        set_note_protected(&s, &dek, "n1", true).unwrap();
        s.set_folder_locked("f", true).unwrap();
        let sealed = content_of(&s, "n1");

        let err = set_note_protected(&s, &dek, "n1", false).unwrap_err();

        assert_eq!(err, "note is protected by its folder");
        assert!(s.note_protected("n1").unwrap());
        assert_eq!(content_of(&s, "n1"), sealed, "still ciphertext at rest");
    }

    #[test]
    fn the_folder_refusal_also_applies_through_an_ancestor() {
        let s = store();
        let dek = Dek::random();
        folder(&s, "top", None);
        folder(&s, "sub", Some("top"));
        seed_in(&s, "n1", "<p>secret</p>", "sub");
        set_note_protected(&s, &dek, "n1", true).unwrap();
        s.set_folder_locked("top", true).unwrap();

        assert_eq!(
            set_note_protected(&s, &dek, "n1", false).unwrap_err(),
            "note is protected by its folder"
        );
    }

    #[test]
    fn unprotecting_with_a_foreign_dek_fails_and_keeps_the_ciphertext() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>secret</p>");
        set_note_protected(&s, &dek, "n1", true).unwrap();
        let sealed = content_of(&s, "n1");

        assert!(set_note_protected(&s, &Dek::random(), "n1", false).is_err());

        assert!(s.note_protected("n1").unwrap());
        assert_eq!(
            content_of(&s, "n1"),
            sealed,
            "a failed decrypt must never blank or corrupt the stored blob"
        );
    }

    #[test]
    fn protecting_a_missing_note_is_an_error() {
        let s = store();
        assert!(set_note_protected(&s, &Dek::random(), "ghost", true).is_err());
        assert!(set_note_protected(&s, &Dek::random(), "ghost", false).is_err());
    }

    #[test]
    fn a_protect_round_trip_is_lossless_for_unicode_and_markup() {
        let s = store();
        let dek = Dek::random();
        let body = "<p>Grüße 🌍</p><p>&lt;escaped&gt;</p>";
        seed(&s, "n1", body);

        set_note_protected(&s, &dek, "n1", true).unwrap();
        set_note_protected(&s, &dek, "n1", false).unwrap();

        assert_eq!(content_of(&s, "n1"), body);
    }
}

#[cfg(test)]
mod folder_lock_tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn locking_encrypts_every_note_in_the_subtree() {
        let s = store();
        let dek = Dek::random();
        folder(&s, "top", None);
        folder(&s, "sub", Some("top"));
        seed_in(&s, "direct", "<p>direct secret</p>", "top");
        seed_in(&s, "nested", "<p>nested secret</p>", "sub");
        seed(&s, "outside", "<p>public</p>");

        set_folder_locked(&s, &dek, "top", true).unwrap();

        assert!(s.folder_locked("top").unwrap());
        for id in ["direct", "nested"] {
            assert!(s.note_protected(id).unwrap(), "{id} should be sealed");
            assert!(!content_of(&s, id).contains("secret"));
        }
        assert!(!s.note_protected("outside").unwrap());
        assert_eq!(content_of(&s, "outside"), "<p>public</p>");
    }

    #[test]
    fn locking_purges_the_subtrees_plaintext_revision_history() {
        let s = store();
        let dek = Dek::random();
        folder(&s, "f", None);
        seed_in(&s, "n1", "<p>secret</p>", "f");
        crate::revisions::add_revision(&s.conn, "n1", "<p>secret</p>", 50).unwrap();

        set_folder_locked(&s, &dek, "f", true).unwrap();

        assert_eq!(revision_count(&s, "n1"), 0);
    }

    #[test]
    fn locking_leaves_an_already_encrypted_note_untouched() {
        let s = store();
        let dek = Dek::random();
        folder(&s, "f", None);
        seed_in(&s, "n1", "<p>secret</p>", "f");
        encrypt_note_in_place(&s, "n1", &dek).unwrap();
        let sealed = content_of(&s, "n1");

        set_folder_locked(&s, &dek, "f", true).unwrap();

        assert_eq!(content_of(&s, "n1"), sealed, "no double encryption");
    }

    #[test]
    fn unlocking_decrypts_the_subtree_again() {
        let s = store();
        let dek = Dek::random();
        folder(&s, "f", None);
        seed_in(&s, "n1", "<p>secret</p>", "f");
        set_folder_locked(&s, &dek, "f", true).unwrap();

        set_folder_locked(&s, &dek, "f", false).unwrap();

        assert!(!s.folder_locked("f").unwrap());
        assert!(!s.note_protected("n1").unwrap());
        assert_eq!(content_of(&s, "n1"), "<p>secret</p>");
    }

    #[test]
    fn unlocking_keeps_notes_sealed_while_another_ancestor_stays_locked() {
        let s = store();
        let dek = Dek::random();
        folder(&s, "top", None);
        folder(&s, "sub", Some("top"));
        seed_in(&s, "n1", "<p>secret</p>", "sub");
        set_folder_locked(&s, &dek, "top", true).unwrap();
        let sealed = content_of(&s, "n1");

        set_folder_locked(&s, &dek, "sub", false).unwrap();

        assert!(
            s.note_protected("n1").unwrap(),
            "the still-locked ancestor keeps the note sealed"
        );
        assert_eq!(content_of(&s, "n1"), sealed);
    }

    #[test]
    fn locking_an_empty_folder_only_flips_the_flag() {
        let s = store();
        let dek = Dek::random();
        folder(&s, "f", None);

        set_folder_locked(&s, &dek, "f", true).unwrap();

        assert!(s.folder_locked("f").unwrap());
    }

    #[test]
    fn locking_marks_the_subtrees_notes_dirty_when_syncing() {
        let s = syncing_store();
        let dek = Dek::random();
        folder(&s, "f", None);
        seed_in(&s, "n1", "<p>secret</p>", "f");
        clear_dirty(&s);

        set_folder_locked(&s, &dek, "f", true).unwrap();

        let dirty = s.load_dirty_notes().unwrap();
        assert_eq!(dirty.len(), 1);
        assert!(dirty[0].protected);
        assert!(!dirty[0].content.contains("secret"));
    }

    #[test]
    fn a_foreign_dek_cannot_unlock_a_folder_and_leaves_the_content_sealed() {
        let s = store();
        let dek = Dek::random();
        folder(&s, "f", None);
        seed_in(&s, "n1", "<p>secret</p>", "f");
        set_folder_locked(&s, &dek, "f", true).unwrap();
        let sealed = content_of(&s, "n1");

        assert!(set_folder_locked(&s, &Dek::random(), "f", false).is_err());

        assert!(s.note_protected("n1").unwrap());
        assert_eq!(content_of(&s, "n1"), sealed);
    }
}

#[cfg(test)]
mod registry_view_tests {
    use super::*;

    fn registry() -> Registry {
        let mut r = Registry::default_for("/data/notefix.db");
        r.rename(&r.active_id.clone(), "Personal".into()).unwrap();
        r.add(
            "b".into(),
            "Work".into(),
            "/data/contexts/b/notefix.db".into(),
        );
        r
    }

    #[test]
    fn to_infos_marks_exactly_one_context_active() {
        let r = registry();
        let infos = to_infos(&r);

        assert_eq!(infos.len(), 2);
        assert_eq!(infos.iter().filter(|i| i.active).count(), 1);
        assert!(infos.iter().find(|i| i.id == r.active_id).unwrap().active);
    }

    #[test]
    fn to_infos_carries_the_server_fields_through() {
        let mut r = Registry::default_for("/d.db");
        r.add_server(
            "s".into(),
            "notes.example".into(),
            "/s.db".into(),
            "https://notes.example".into(),
        );
        r.bind_workspace("s", "ws-1".into()).unwrap();

        let info = to_infos(&r).into_iter().find(|i| i.id == "s").unwrap();
        assert_eq!(info.kind, "server");
        assert_eq!(info.server_url, "https://notes.example");
        assert_eq!(info.workspace_id, "ws-1");
        assert!(!info.active);
    }

    #[test]
    fn registry_contexts_mirrors_every_entry() {
        let r = registry();
        let ctxs = registry_contexts(&r);

        assert_eq!(ctxs.len(), 2);
        let labels: Vec<&str> = ctxs.iter().map(|c| c.label.as_str()).collect();
        assert!(labels.contains(&"Personal") && labels.contains(&"Work"));
        assert!(ctxs.iter().all(|c| c.kind == "local"));
    }

    #[test]
    fn active_server_is_none_for_a_local_context() {
        let r = registry();
        assert!(active_server(&r).is_none());
    }

    #[test]
    fn active_server_returns_the_entry_when_the_active_context_is_server_backed() {
        let mut r = Registry::default_for("/d.db");
        r.add_server("s".into(), "srv".into(), "/s.db".into(), "https://s".into());
        r.set_active("s").unwrap();

        let ctx = active_server(&r).unwrap();
        assert_eq!(ctx.id, "s");
        assert_eq!(ctx.server_url, "https://s");
    }
}

#[cfg(test)]
mod context_mutation_tests {
    use super::*;

    struct Fixture {
        _dir: tempfile::TempDir,
        contexts: PathBuf,
        profiles: PathBuf,
        reg: Registry,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let contexts = dir.path().join("contexts");
        let profiles = dir.path().join("profiles.json");
        let reg = Registry::default_for(&dir.path().join("notefix.db").to_string_lossy());
        Fixture {
            _dir: dir,
            contexts,
            profiles,
            reg,
        }
    }

    fn saved(f: &Fixture) -> Registry {
        let json = std::fs::read_to_string(&f.profiles).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn prepare_context_db_creates_a_migrated_database() {
        let dir = tempfile::tempdir().unwrap();

        let path = prepare_context_db(dir.path(), "ctx-1").unwrap();

        assert_eq!(path, dir.path().join("ctx-1").join("notefix.db"));
        assert!(path.is_file());
        // Migrated: the settings table the app relies on exists.
        let s = Store::open(&path).unwrap();
        assert!(crate::settings::load_settings(&s.conn).is_ok());
    }

    #[test]
    fn context_add_creates_an_isolated_db_and_activates_it() {
        let mut f = fixture();
        let before = f.reg.active_id.clone();

        let (path, infos) =
            context_add(&mut f.reg, &f.contexts, &f.profiles, "Work".into()).unwrap();

        assert!(path.is_file());
        assert!(
            path.parent().unwrap().parent().unwrap() == f.contexts,
            "each context gets its own directory so images stay isolated"
        );
        assert_eq!(infos.len(), 2);
        assert_ne!(f.reg.active_id, before, "the new context becomes active");
        assert_eq!(f.reg.active().unwrap().label, "Work");
        assert_eq!(saved(&f).active_id, f.reg.active_id, "registry persisted");
    }

    #[test]
    fn two_added_contexts_get_separate_directories() {
        let mut f = fixture();
        let (a, _) = context_add(&mut f.reg, &f.contexts, &f.profiles, "A".into()).unwrap();
        let (b, _) = context_add(&mut f.reg, &f.contexts, &f.profiles, "B".into()).unwrap();

        assert_ne!(a.parent(), b.parent());
        assert_eq!(f.reg.contexts.len(), 3);
    }

    #[test]
    fn context_switch_returns_the_path_and_sync_flag() {
        let mut f = fixture();
        f.reg.add_server(
            "s".into(),
            "srv".into(),
            "/tmp/s.db".into(),
            "https://s".into(),
        );

        let (path, is_server) = context_switch(&mut f.reg, &f.profiles, "s").unwrap();

        assert_eq!(path, "/tmp/s.db");
        assert!(is_server);
        assert_eq!(saved(&f).active_id, "s");
    }

    #[test]
    fn switching_to_a_local_context_reports_no_sync() {
        let mut f = fixture();
        f.reg.add("b".into(), "B".into(), "/tmp/b.db".into());

        let (_path, is_server) = context_switch(&mut f.reg, &f.profiles, "b").unwrap();

        assert!(!is_server);
    }

    #[test]
    fn switching_to_an_unknown_context_is_rejected_and_persists_nothing() {
        let mut f = fixture();
        let before = f.reg.active_id.clone();

        assert_eq!(
            context_switch(&mut f.reg, &f.profiles, "nope").unwrap_err(),
            "unknown context"
        );

        assert_eq!(f.reg.active_id, before);
        assert!(!f.profiles.exists(), "nothing was written");
    }

    #[test]
    fn context_rename_updates_the_label_and_persists() {
        let mut f = fixture();
        let id = f.reg.active_id.clone();

        let infos = context_rename(&mut f.reg, &f.profiles, &id, "Renamed".into()).unwrap();

        assert_eq!(infos[0].label, "Renamed");
        assert_eq!(saved(&f).active().unwrap().label, "Renamed");
    }

    #[test]
    fn renaming_an_unknown_context_is_rejected() {
        let mut f = fixture();
        assert_eq!(
            context_rename(&mut f.reg, &f.profiles, "nope", "X".into()).unwrap_err(),
            "unknown context"
        );
    }

    #[test]
    fn context_remove_drops_the_entry_and_can_keep_the_file() {
        let mut f = fixture();
        let (path, _) = context_add(&mut f.reg, &f.contexts, &f.profiles, "Doomed".into()).unwrap();
        // The added context became active; switch away so it can be removed.
        let other = f.reg.contexts[0].id.clone();
        let doomed = f.reg.active_id.clone();
        context_switch(&mut f.reg, &f.profiles, &other).unwrap();

        let (removed, infos) = context_remove(&mut f.reg, &f.profiles, &doomed, false).unwrap();

        assert_eq!(removed.id, doomed);
        assert_eq!(infos.len(), 1);
        assert!(path.is_file(), "delete_file = false keeps the database");
        assert_eq!(saved(&f).contexts.len(), 1);
    }

    #[test]
    fn context_remove_can_delete_the_database_and_its_sidecars() {
        let mut f = fixture();
        let (path, _) = context_add(&mut f.reg, &f.contexts, &f.profiles, "Doomed".into()).unwrap();
        for ext in ["-wal", "-shm"] {
            std::fs::write(with_ext(&path, ext), b"x").unwrap();
        }
        let other = f.reg.contexts[0].id.clone();
        let doomed = f.reg.active_id.clone();
        context_switch(&mut f.reg, &f.profiles, &other).unwrap();

        context_remove(&mut f.reg, &f.profiles, &doomed, true).unwrap();

        assert!(!path.exists());
        assert!(!with_ext(&path, "-wal").exists());
        assert!(!with_ext(&path, "-shm").exists());
    }

    #[test]
    fn the_active_context_cannot_be_removed() {
        let mut f = fixture();
        let active = f.reg.active_id.clone();
        f.reg.add("b".into(), "B".into(), "/tmp/b.db".into());

        assert_eq!(
            context_remove(&mut f.reg, &f.profiles, &active, false).unwrap_err(),
            "cannot remove active context"
        );
        assert_eq!(f.reg.contexts.len(), 2, "nothing was removed");
        assert!(!f.profiles.exists(), "and nothing was persisted");
    }

    #[test]
    fn the_last_remaining_context_cannot_be_removed() {
        let mut f = fixture();
        f.reg.add("b".into(), "B".into(), "/tmp/b.db".into());
        f.reg.set_active("b").unwrap();
        let only_other = f.reg.contexts[0].id.clone();
        // Remove the non-active one so a single (active) context is left...
        context_remove(&mut f.reg, &f.profiles, &only_other, false).unwrap();
        assert_eq!(f.reg.contexts.len(), 1);

        // ...which is then unremovable, active-check first.
        assert_eq!(
            context_remove(&mut f.reg, &f.profiles, "b", false).unwrap_err(),
            "cannot remove active context"
        );
    }

    #[test]
    fn removing_an_unknown_context_is_rejected() {
        let mut f = fixture();
        f.reg.add("b".into(), "B".into(), "/tmp/b.db".into());

        assert_eq!(
            context_remove(&mut f.reg, &f.profiles, "nope", false).unwrap_err(),
            "unknown context"
        );
        assert_eq!(f.reg.contexts.len(), 2);
    }

    #[test]
    fn context_bind_workspace_binds_and_optionally_renames() {
        let mut f = fixture();
        f.reg.add_server(
            "s".into(),
            "srv".into(),
            "/tmp/s.db".into(),
            "https://s".into(),
        );

        let infos =
            context_bind_workspace(&mut f.reg, &f.profiles, "s", "ws-9".into(), "Team".into())
                .unwrap();

        let info = infos.into_iter().find(|i| i.id == "s").unwrap();
        assert_eq!(info.workspace_id, "ws-9");
        assert_eq!(info.label, "Team");
        assert_eq!(saved(&f).contexts.last().unwrap().workspace_id, "ws-9");
    }

    #[test]
    fn an_empty_label_leaves_the_existing_name_alone() {
        let mut f = fixture();
        f.reg.add_server(
            "s".into(),
            "Original".into(),
            "/tmp/s.db".into(),
            "https://s".into(),
        );

        context_bind_workspace(&mut f.reg, &f.profiles, "s", "ws-9".into(), String::new()).unwrap();

        assert_eq!(
            f.reg.contexts.iter().find(|c| c.id == "s").unwrap().label,
            "Original"
        );
    }

    #[test]
    fn binding_an_unknown_context_is_rejected() {
        let mut f = fixture();
        assert_eq!(
            context_bind_workspace(&mut f.reg, &f.profiles, "nope", "ws".into(), String::new())
                .unwrap_err(),
            "unknown context"
        );
    }

    #[test]
    fn register_server_context_adds_it_as_the_active_server_entry() {
        let mut f = fixture();

        let infos = register_server_context(
            &mut f.reg,
            &f.profiles,
            "srv-1",
            "notes.example".into(),
            Path::new("/tmp/srv.db"),
            "https://notes.example".into(),
        )
        .unwrap();

        let info = infos.into_iter().find(|i| i.id == "srv-1").unwrap();
        assert!(info.active);
        assert_eq!(info.kind, "server");
        assert_eq!(info.server_url, "https://notes.example");
        assert_eq!(info.workspace_id, "", "not bound to a workspace yet");
        assert_eq!(saved(&f).active_id, "srv-1");
    }
}

#[cfg(test)]
mod auth_flow_tests {
    use super::*;

    fn config() -> crate::auth::OAuthConfig {
        crate::auth::OAuthConfig {
            client_id: "notefix-desktop".into(),
            authorize_url: "https://notes.example/oauth/authorize".into(),
            token_url: "https://notes.example/oauth/token".into(),
            scopes: vec!["notes.read".into(), "notes.write".into()],
        }
    }

    #[test]
    fn server_label_uses_the_host() {
        assert_eq!(server_label("https://notes.example/api"), "notes.example");
        assert_eq!(server_label("http://localhost:8000"), "localhost");
    }

    #[test]
    fn server_label_falls_back_to_the_raw_string_when_unparsable() {
        assert_eq!(server_label("not a url"), "not a url");
        assert_eq!(server_label(""), "");
    }

    #[test]
    fn authorize_url_carries_every_pkce_parameter() {
        let url = build_authorize_url(&config(), "CHAL", "STATE").unwrap();
        let parsed = url::Url::parse(&url).unwrap();
        let q: std::collections::HashMap<String, String> =
            parsed.query_pairs().into_owned().collect();

        assert_eq!(q["response_type"], "code");
        assert_eq!(q["client_id"], "notefix-desktop");
        assert_eq!(q["redirect_uri"], crate::auth::REDIRECT_URI);
        assert_eq!(q["code_challenge"], "CHAL");
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["state"], "STATE");
        assert_eq!(q["scope"], "notes.read notes.write");
        assert!(url.starts_with("https://notes.example/oauth/authorize?"));
    }

    #[test]
    fn no_scopes_means_no_scope_parameter() {
        let mut c = config();
        c.scopes.clear();

        let url = build_authorize_url(&c, "CHAL", "STATE").unwrap();

        assert!(!url.contains("scope="));
    }

    #[test]
    fn an_unparsable_authorize_url_is_rejected() {
        let mut c = config();
        c.authorize_url = "/relative/only".into();
        assert!(build_authorize_url(&c, "CHAL", "STATE").is_err());
    }

    #[test]
    fn existing_query_parameters_are_preserved() {
        let mut c = config();
        c.authorize_url = "https://notes.example/authorize?tenant=acme".into();

        let url = build_authorize_url(&c, "CHAL", "STATE").unwrap();

        let parsed = url::Url::parse(&url).unwrap();
        let q: std::collections::HashMap<String, String> =
            parsed.query_pairs().into_owned().collect();
        assert_eq!(q["tenant"], "acme");
        assert_eq!(q["state"], "STATE");
    }

    #[test]
    fn the_callback_yields_the_code_and_state() {
        let (code, state) = parse_auth_callback("notefix://auth?code=abc123&state=xyz789").unwrap();
        assert_eq!(code, "abc123");
        assert_eq!(state, "xyz789");
    }

    #[test]
    fn percent_encoded_callback_values_are_decoded() {
        let (code, state) =
            parse_auth_callback("notefix://auth?code=a%2Bb%2Fc&state=s%20t").unwrap();
        assert_eq!(code, "a+b/c");
        assert_eq!(state, "s t");
    }

    #[test]
    fn a_callback_missing_code_or_state_is_rejected() {
        assert_eq!(
            parse_auth_callback("notefix://auth?state=xyz").unwrap_err(),
            "missing code in callback"
        );
        assert_eq!(
            parse_auth_callback("notefix://auth?code=abc").unwrap_err(),
            "missing state in callback"
        );
        assert_eq!(
            parse_auth_callback("notefix://auth").unwrap_err(),
            "missing code in callback"
        );
    }

    #[test]
    fn an_unparsable_callback_url_is_rejected() {
        assert!(parse_auth_callback("://////").is_err());
        assert!(parse_auth_callback("").is_err());
    }

    #[test]
    fn extra_callback_parameters_are_ignored() {
        let (code, state) =
            parse_auth_callback("notefix://auth?foo=1&code=c&bar=2&state=s&baz=3").unwrap();
        assert_eq!((code.as_str(), state.as_str()), ("c", "s"));
    }
}

#[cfg(test)]
mod sync_status_tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn a_local_context_reports_local_without_touching_the_store() {
        let r = Registry::default_for("/d.db");
        let status = sync_status_local(&r).unwrap();
        assert_eq!(status.state, "local");
        assert_eq!(status.pending, 0);
        assert_eq!(status.last_synced_at, 0);
    }

    #[test]
    fn an_unbound_server_context_reports_unbound() {
        let mut r = Registry::default_for("/d.db");
        r.add_server("s".into(), "srv".into(), "/s.db".into(), "https://s".into());
        r.set_active("s").unwrap();

        let status = sync_status_local(&r).unwrap();
        assert_eq!(status.state, "unbound");
    }

    #[test]
    fn a_bound_server_context_defers_to_the_store() {
        let mut r = Registry::default_for("/d.db");
        r.add_server("s".into(), "srv".into(), "/s.db".into(), "https://s".into());
        r.set_active("s").unwrap();
        r.bind_workspace("s", "ws-1".into()).unwrap();

        assert!(sync_status_local(&r).is_none());
    }

    #[test]
    fn a_store_that_never_synced_reports_syncing() {
        let s = syncing_store();
        let status = sync_status_synced(&s).unwrap();
        assert_eq!(status.state, "syncing");
        assert_eq!(status.last_synced_at, 0);
    }

    #[test]
    fn a_store_with_a_cursor_reports_synced() {
        let s = syncing_store();
        crate::migrate::set_meta_i64(&s.conn, "sync_last_at", 1_700_000).unwrap();

        let status = sync_status_synced(&s).unwrap();
        assert_eq!(status.state, "synced");
        assert_eq!(status.last_synced_at, 1_700_000);
    }

    #[test]
    fn pending_counts_dirty_notes_and_folders_together() {
        let s = syncing_store();
        seed(&s, "n1", "<p>a</p>");
        seed(&s, "n2", "<p>b</p>");
        crate::folders::create_folder(&s.conn, "f1", "F", None).unwrap();
        crate::folders::touch_folder(&s.conn, "f1").unwrap();

        assert_eq!(sync_status_synced(&s).unwrap().pending, 3);
    }

    #[test]
    fn nothing_dirty_means_zero_pending() {
        let s = syncing_store();
        seed(&s, "n1", "<p>a</p>");
        clear_dirty(&s);

        assert_eq!(sync_status_synced(&s).unwrap().pending, 0);
    }
}

#[cfg(test)]
mod select_notes_tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn an_empty_selection_means_everything() {
        let all = vec![note("a", ""), note("b", "")];
        assert_eq!(select_notes(all, &[]).len(), 2);
    }

    #[test]
    fn a_selection_keeps_only_the_named_notes_in_input_order() {
        let all = vec![note("a", ""), note("b", ""), note("c", "")];
        let picked = select_notes(all, &["c".into(), "a".into()]);
        let ids: Vec<&str> = picked.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(ids, vec!["a", "c"], "input order wins, not selection order");
    }

    #[test]
    fn unknown_ids_in_the_selection_are_ignored() {
        let all = vec![note("a", "")];
        assert!(select_notes(all, &["ghost".into()]).is_empty());
    }
}

#[cfg(test)]
mod export_tests {
    use super::test_support::*;
    use super::*;

    /// An images root holding one 1x1-ish PNG at `a/b/pic.png`.
    fn images_root(dir: &Path) -> PathBuf {
        let root = dir.join("images");
        std::fs::create_dir_all(root.join("a/b")).unwrap();
        std::fs::write(root.join("a/b/pic.png"), [0x89, 0x50, 0x4e, 0x47]).unwrap();
        root
    }

    #[test]
    fn export_notes_json_writes_every_note_when_no_ids_are_given() {
        let dir = tempfile::tempdir().unwrap();
        let s = store();
        seed(&s, "a", "<p>alpha</p>");
        seed(&s, "b", "<p>beta</p>");
        let out = dir.path().join("notes.json");

        export_notes_json(&s.load_notes().unwrap(), &out, &[]).unwrap();

        let json = std::fs::read_to_string(&out).unwrap();
        assert!(json.contains("alpha") && json.contains("beta"));
        assert!(json.contains("\"updatedAt\""), "camelCase wire shape");
    }

    #[test]
    fn export_notes_json_honors_an_id_selection() {
        let dir = tempfile::tempdir().unwrap();
        let s = store();
        seed(&s, "a", "<p>alpha</p>");
        seed(&s, "b", "<p>beta</p>");
        let out = dir.path().join("notes.json");

        export_notes_json(&s.load_notes().unwrap(), &out, &["b".into()]).unwrap();

        let json = std::fs::read_to_string(&out).unwrap();
        assert!(json.contains("beta"));
        assert!(!json.contains("alpha"));
    }

    #[test]
    fn export_notes_json_reports_an_unwritable_target() {
        let s = store();
        let err = export_notes_json(
            &s.load_notes().unwrap(),
            Path::new("/no/such/dir/notes.json"),
            &[],
        )
        .unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn export_notes_inlined_turns_images_into_data_urls() {
        let dir = tempfile::tempdir().unwrap();
        let root = images_root(dir.path());
        let s = store();
        seed(&s, "a", "<img src=\"noteimg://localhost/a/b/pic.png\">");
        let out = dir.path().join("notes.json");

        export_notes_inlined(s.load_notes().unwrap(), &root, &out, &[]).unwrap();

        let json = std::fs::read_to_string(&out).unwrap();
        assert!(json.contains("data:image/png;base64,"));
        assert!(!json.contains("noteimg://"));
    }

    #[test]
    fn a_missing_image_leaves_its_url_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("images");
        std::fs::create_dir_all(&root).unwrap();
        let s = store();
        seed(&s, "a", "<img src=\"noteimg://localhost/gone.png\">");
        let out = dir.path().join("notes.json");

        export_notes_inlined(s.load_notes().unwrap(), &root, &out, &[]).unwrap();

        let json = std::fs::read_to_string(&out).unwrap();
        assert!(json.contains("noteimg://localhost/gone.png"));
    }

    #[test]
    fn note_inlined_html_returns_one_notes_html_with_images_embedded() {
        let dir = tempfile::tempdir().unwrap();
        let root = images_root(dir.path());
        let s = store();
        seed(
            &s,
            "a",
            "<p>hi</p><img src=\"noteimg://localhost/a/b/pic.png\">",
        );

        let html = note_inlined_html(s.load_all_notes().unwrap(), &root, "a").unwrap();

        assert!(html.starts_with("<p>hi</p>"));
        assert!(html.contains("data:image/png;base64,"));
    }

    #[test]
    fn note_inlined_html_rejects_an_unknown_note() {
        let dir = tempfile::tempdir().unwrap();
        let s = store();
        assert_eq!(
            note_inlined_html(s.load_all_notes().unwrap(), dir.path(), "ghost").unwrap_err(),
            "note not found"
        );
    }

    #[test]
    fn note_inlined_html_can_still_reach_a_trashed_note() {
        // It reads `load_all_notes`, so printing a note that was just trashed
        // in another window still works.
        let dir = tempfile::tempdir().unwrap();
        let s = store();
        seed(&s, "a", "<p>trashed</p>");
        s.trash_note("a", 1).unwrap();

        assert_eq!(
            note_inlined_html(s.load_all_notes().unwrap(), dir.path(), "a").unwrap(),
            "<p>trashed</p>"
        );
    }

    #[test]
    fn save_export_writes_the_bytes_verbatim() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.pdf");

        save_export(&out, b"%PDF-1.4 fake").unwrap();

        assert_eq!(std::fs::read(&out).unwrap(), b"%PDF-1.4 fake");
    }

    #[test]
    fn save_export_reports_a_bad_destination() {
        assert!(save_export(Path::new("/no/such/dir/out.bin"), b"x").is_err());
    }

    #[test]
    fn export_md_bundle_writes_the_markdown_and_copies_its_images() {
        let dir = tempfile::tempdir().unwrap();
        let root = images_root(dir.path());
        let dest = dir.path().join("bundle");

        export_md_bundle(
            &root,
            &dest,
            "# Title\n\n![](noteimg://localhost/a/b/pic.png)\n",
            "My Note",
        )
        .unwrap();

        let md = std::fs::read_to_string(dest.join("My Note.md")).unwrap();
        assert!(md.contains("images/a/b/pic.png"));
        assert!(!md.contains("noteimg://"));
        assert!(dest.join("images/a/b/pic.png").is_file());
    }

    #[test]
    fn export_md_bundle_sanitizes_path_characters_in_the_filename() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("bundle");

        export_md_bundle(dir.path(), &dest, "# x", "a/b:c\\d").unwrap();

        assert!(dest.join("a-b-c-d.md").is_file());
    }

    #[test]
    fn export_notes_bundle_writes_notes_json_next_to_the_images() {
        let dir = tempfile::tempdir().unwrap();
        let root = images_root(dir.path());
        let dest = dir.path().join("bundle");
        let s = store();
        seed(&s, "a", "<img src=\"noteimg://localhost/a/b/pic.png\">");
        seed(&s, "b", "<p>no images</p>");

        export_notes_bundle(s.load_notes().unwrap(), &root, &dest, &[]).unwrap();

        let json = std::fs::read_to_string(dest.join("notes.json")).unwrap();
        assert!(json.contains("images/a/b/pic.png"));
        assert!(!json.contains("noteimg://"));
        assert!(json.contains("no images"));
        assert!(dest.join("images/a/b/pic.png").is_file());
    }

    #[test]
    fn export_notes_bundle_honors_an_id_selection() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("bundle");
        let s = store();
        seed(&s, "a", "<p>alpha</p>");
        seed(&s, "b", "<p>beta</p>");

        export_notes_bundle(s.load_notes().unwrap(), dir.path(), &dest, &["a".into()]).unwrap();

        let json = std::fs::read_to_string(dest.join("notes.json")).unwrap();
        assert!(json.contains("alpha"));
        assert!(!json.contains("beta"));
    }

    #[test]
    fn a_protected_notes_body_is_exported_as_ciphertext() {
        // Export runs without the vault, so a sealed note must stay sealed in
        // the exported file. Its `title` is plaintext metadata by design (see
        // `Note::title`) and is therefore expected to be readable.
        let dir = tempfile::tempdir().unwrap();
        let s = store();
        seed(&s, "a", "<p>Expenses</p><p>classified body</p>");
        encrypt_note_in_place(&s, "a", &Dek::random()).unwrap();
        let out = dir.path().join("notes.json");

        export_notes_json(&s.load_notes().unwrap(), &out, &[]).unwrap();

        let json = std::fs::read_to_string(&out).unwrap();
        assert!(
            !json.contains("classified body"),
            "the sealed body must never reach the export file"
        );
        assert!(
            json.contains("Expenses"),
            "the plaintext title is by design"
        );
    }
}

#[cfg(test)]
mod save_image_tests {
    use super::*;

    #[test]
    fn writes_the_file_under_the_notes_shard_and_returns_its_url() {
        let dir = tempfile::tempdir().unwrap();

        let url = save_image(dir.path(), "aa-bb-cc", "pic.png", b"binary").unwrap();

        assert_eq!(url, "noteimg://localhost/aa/bb/cc/pic.png");
        assert_eq!(
            std::fs::read(dir.path().join("aa/bb/cc/pic.png")).unwrap(),
            b"binary"
        );
    }

    #[test]
    fn a_note_id_without_dashes_is_a_single_directory() {
        let dir = tempfile::tempdir().unwrap();

        let url = save_image(dir.path(), "plainid", "x.png", b"x").unwrap();

        assert_eq!(url, "noteimg://localhost/plainid/x.png");
        assert!(dir.path().join("plainid/x.png").is_file());
    }

    #[test]
    fn a_traversing_or_empty_name_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        for bad in ["", "../escape.png", "a/../b.png", "back\\slash.png"] {
            assert_eq!(
                save_image(dir.path(), "n1", bad, b"x").unwrap_err(),
                "invalid name",
                "name {bad:?} must be rejected"
            );
        }
        assert!(
            !dir.path().join("escape.png").exists(),
            "nothing escaped the images root"
        );
    }

    #[test]
    fn a_traversing_or_empty_note_id_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        for bad in ["", "..", "a/../b"] {
            assert_eq!(
                save_image(dir.path(), bad, "pic.png", b"x").unwrap_err(),
                "invalid note id",
                "note id {bad:?} must be rejected"
            );
        }
    }

    #[test]
    fn the_name_is_validated_before_the_note_id() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            save_image(dir.path(), "..", "../x.png", b"x").unwrap_err(),
            "invalid name"
        );
    }

    #[test]
    fn overwriting_an_existing_image_replaces_its_bytes() {
        let dir = tempfile::tempdir().unwrap();
        save_image(dir.path(), "n1", "pic.png", b"first").unwrap();
        save_image(dir.path(), "n1", "pic.png", b"second").unwrap();

        assert_eq!(
            std::fs::read(dir.path().join("n1/pic.png")).unwrap(),
            b"second"
        );
    }
}

#[cfg(test)]
mod check_paths_tests {
    use super::*;

    #[test]
    fn reports_both_directories_as_writable() {
        let dir = tempfile::tempdir().unwrap();
        let images = dir.path().join("images");
        std::fs::create_dir_all(&images).unwrap();
        let db = dir.path().join("notefix.db");

        let checks = check_paths(&db, &images);

        assert!(checks.db_writable);
        assert!(checks.images_writable);
        assert_eq!(checks.db_path, dir.path().to_string_lossy());
        assert_eq!(checks.images_path, images.to_string_lossy());
    }

    #[test]
    fn a_missing_images_directory_is_not_writable() {
        let dir = tempfile::tempdir().unwrap();
        let checks = check_paths(&dir.path().join("notefix.db"), &dir.path().join("nope"));

        assert!(checks.db_writable);
        assert!(!checks.images_writable);
    }

    #[test]
    fn a_database_path_with_no_parent_is_not_writable() {
        let checks = check_paths(Path::new("/"), Path::new("/definitely/not/here"));
        assert!(!checks.db_writable);
        assert!(!checks.images_writable);
    }
}

#[cfg(test)]
mod db_location_tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn with_ext_appends_sidecar_suffixes_and_passes_the_bare_path_through() {
        let p = Path::new("/data/notefix.db");
        assert_eq!(with_ext(p, ""), PathBuf::from("/data/notefix.db"));
        assert_eq!(with_ext(p, "-wal"), PathBuf::from("/data/notefix.db-wal"));
        assert_eq!(with_ext(p, "-shm"), PathBuf::from("/data/notefix.db-shm"));
    }

    #[test]
    fn move_db_files_relocates_the_database_and_its_sidecars() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("old.db");
        let to = dir.path().join("sub").join("new.db");
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::write(&from, b"db").unwrap();
        std::fs::write(with_ext(&from, "-wal"), b"wal").unwrap();
        std::fs::write(with_ext(&from, "-shm"), b"shm").unwrap();

        move_db_files(&from, &to).unwrap();

        assert_eq!(std::fs::read(&to).unwrap(), b"db");
        assert_eq!(std::fs::read(with_ext(&to, "-wal")).unwrap(), b"wal");
        assert_eq!(std::fs::read(with_ext(&to, "-shm")).unwrap(), b"shm");
        assert!(!from.exists());
        assert!(!with_ext(&from, "-wal").exists());
    }

    #[test]
    fn move_db_files_skips_sidecars_that_do_not_exist() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("old.db");
        let to = dir.path().join("new.db");
        std::fs::write(&from, b"db").unwrap();

        move_db_files(&from, &to).unwrap();

        assert!(to.is_file());
        assert!(!with_ext(&to, "-wal").exists());
    }

    #[test]
    fn move_db_files_is_a_no_op_when_the_source_is_absent() {
        let dir = tempfile::tempdir().unwrap();
        move_db_files(&dir.path().join("missing.db"), &dir.path().join("new.db")).unwrap();
        assert!(!dir.path().join("new.db").exists());
    }

    #[test]
    fn move_db_files_reports_an_unreachable_destination() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("old.db");
        std::fs::write(&from, b"db").unwrap();

        assert!(move_db_files(&from, Path::new("/no/such/dir/new.db")).is_err());
    }

    #[test]
    fn reopen_store_at_points_the_store_at_a_migrated_database() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("moved.db");
        let mut s = store();
        seed(&s, "in-memory-only", "<p>x</p>");

        reopen_store_at(&mut s, &target).unwrap();

        assert!(target.is_file());
        assert!(
            s.load_notes().unwrap().is_empty(),
            "the store now reads the new database"
        );
        // Migrated, so it is immediately usable.
        seed(&s, "fresh", "<p>y</p>");
        assert_eq!(s.load_notes().unwrap().len(), 1);
    }

    #[test]
    fn reopen_store_at_reports_an_unopenable_path() {
        let mut s = store();
        assert!(reopen_store_at(&mut s, Path::new("/no/such/dir/x.db")).is_err());
    }

    #[test]
    fn point_active_context_at_only_moves_the_active_entry() {
        let mut r = Registry::default_for("/old/notefix.db");
        let active = r.active_id.clone();
        r.add("other".into(), "Other".into(), "/other/notefix.db".into());

        point_active_context_at(&mut r, Path::new("/new/notefix.db"));

        assert_eq!(r.active().unwrap().path, "/new/notefix.db");
        assert_eq!(
            r.contexts.iter().find(|c| c.id == "other").unwrap().path,
            "/other/notefix.db"
        );
        assert_eq!(r.active_id, active, "the active context does not change");
    }

    #[test]
    fn point_active_context_at_is_a_no_op_when_the_active_id_is_dangling() {
        let mut r = Registry::default_for("/old/notefix.db");
        r.active_id = "gone".into();

        point_active_context_at(&mut r, Path::new("/new/notefix.db"));

        assert_eq!(r.contexts[0].path, "/old/notefix.db");
    }
}

#[cfg(test)]
mod sync_cycle_tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn collect_sync_push_snapshots_every_dirty_row() {
        let s = syncing_store();
        seed(&s, "n1", "<p>a</p>");
        crate::folders::create_folder(&s.conn, "f1", "F", None).unwrap();
        crate::folders::touch_folder(&s.conn, "f1").unwrap();

        let push = collect_sync_push(&s).unwrap();

        assert_eq!(push.notes.len(), 1);
        assert_eq!(push.folders.len(), 1);
        assert_eq!(push.note_ids.len(), 1);
        assert_eq!(push.note_ids[0].0, "n1");
        assert_eq!(push.folder_ids[0].0, "f1");
        assert_eq!(push.since, 0, "no cursor yet on a fresh context");
        assert_eq!(
            push.notes[0]["id"], "n1",
            "already in the server wire shape"
        );
    }

    #[test]
    fn collect_sync_push_is_empty_when_nothing_is_dirty() {
        let s = syncing_store();
        seed(&s, "n1", "<p>a</p>");
        clear_dirty(&s);

        let push = collect_sync_push(&s).unwrap();

        assert!(push.notes.is_empty() && push.note_ids.is_empty());
        assert!(push.folders.is_empty() && push.folder_ids.is_empty());
    }

    #[test]
    fn collect_sync_push_resumes_from_the_stored_cursor() {
        let s = syncing_store();
        crate::migrate::set_meta_i64(&s.conn, "sync_cursor", 4_242).unwrap();

        assert_eq!(collect_sync_push(&s).unwrap().since, 4_242);
    }

    #[test]
    fn committing_clears_the_pushed_rows_and_advances_both_markers() {
        let s = syncing_store();
        seed(&s, "n1", "<p>a</p>");
        let push = collect_sync_push(&s).unwrap();

        commit_sync_result(&s, &push.note_ids, &push.folder_ids, &[], &[], 99, 1_700).unwrap();

        assert!(s.load_dirty_notes().unwrap().is_empty());
        assert_eq!(crate::migrate::get_meta_i64(&s.conn, "sync_cursor", 0), 99);
        assert_eq!(
            crate::migrate::get_meta_i64(&s.conn, "sync_last_at", 0),
            1_700
        );
    }

    #[test]
    fn a_row_re_edited_during_the_network_window_stays_queued() {
        // The (id, updated_at) snapshot is what makes this safe: the edit that
        // landed mid-flight bumped `updated_at`, so the clear must not match.
        let s = syncing_store();
        seed(&s, "n1", "<p>a</p>");
        let push = collect_sync_push(&s).unwrap();

        // …network window… the user edits the note again.
        s.set_content_silent("n1", "<p>edited mid-flight</p>")
            .unwrap();
        s.conn
            .execute(
                "UPDATE notes SET updated_at = ?2, dirty = 1 WHERE id = ?1",
                ("n1", push.note_ids[0].1 + 1),
            )
            .unwrap();

        commit_sync_result(&s, &push.note_ids, &push.folder_ids, &[], &[], 5, 1).unwrap();

        let still_dirty = s.load_dirty_notes().unwrap();
        assert_eq!(still_dirty.len(), 1, "the re-edit is pushed next cycle");
        assert_eq!(still_dirty[0].content, "<p>edited mid-flight</p>");
    }

    #[test]
    fn committing_merges_the_pulled_rows_into_the_local_cache() {
        let s = syncing_store();
        let pulled_note = serde_json::json!({
            "id": "server-note",
            "content": "<p>from the server</p>",
            "updatedAt": 5_000,
            "folderId": serde_json::Value::Null,
            "pinned": false,
            "archived": false,
            "color": "",
            "dueAt": serde_json::Value::Null,
            "position": 0,
            "deletedAt": serde_json::Value::Null,
            "protected": false,
            "title": "from the server",
        });

        commit_sync_result(&s, &[], &[], &[], &[pulled_note], 7, 1).unwrap();

        let notes = s.load_notes().unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id, "server-note");
        assert_eq!(notes[0].content, "<p>from the server</p>");
        assert!(!notes[0].dirty, "a pulled row arrives clean");
    }

    #[test]
    fn locally_present_images_keeps_only_files_that_exist() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
        std::fs::write(dir.path().join("a/b/here.png"), b"x").unwrap();
        let referenced: std::collections::HashSet<String> = ["a/b/here.png", "a/b/gone.png"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let present = locally_present_images(&referenced, dir.path());

        assert_eq!(present.len(), 1);
        assert!(present.contains("a/b/here.png"));
    }

    #[test]
    fn locally_present_images_treats_a_traversing_path_as_absent() {
        // Even if `../secret` exists on disk, it must never be reported as a
        // locally present image — that would upload a file outside the root.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("images");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(dir.path().join("secret.txt"), b"x").unwrap();
        let referenced: std::collections::HashSet<String> =
            ["../secret.txt".to_string(), "".to_string()]
                .into_iter()
                .collect();

        assert!(locally_present_images(&referenced, &root).is_empty());
    }

    #[test]
    fn locally_present_images_of_nothing_is_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(locally_present_images(&Default::default(), dir.path()).is_empty());
    }
}

/// A dropped table is the cheapest way to make every `rusqlite` call in an op
/// fail. These prove the ops surface a database failure as an `Err(String)`
/// rather than panicking or, worse, silently reporting success.
#[cfg(test)]
mod database_error_tests {
    use super::test_support::*;
    use super::*;

    fn store_without_notes() -> Store {
        let s = store();
        s.conn.execute_batch("DROP TABLE notes;").unwrap();
        s
    }

    #[test]
    fn note_reads_report_the_failure() {
        let s = store_without_notes();
        assert!(load_note_content(&s, None, "n1").is_err());
        assert!(search_notes(&s, "x", true).is_err());
        assert!(note_stats(&s).is_err());
    }

    #[test]
    fn note_writes_report_the_failure() {
        let s = store_without_notes();
        assert!(save_note(&s, None, &note("n1", "<p>x</p>")).is_err());
        assert!(delete_note(&s, "n1", 1).is_err());
        assert!(reconcile_folder_move(&s, "n1", None, None).is_err());
        assert!(reconcile_reorder(&s, None, &["n1".into()], None).is_err());
    }

    #[test]
    fn protection_transitions_report_the_failure() {
        let s = store_without_notes();
        let dek = Dek::random();
        assert!(set_note_protected(&s, &dek, "n1", true).is_err());
        assert!(encrypt_note_in_place(&s, "n1", &dek).is_err());
    }

    #[test]
    fn exports_report_an_unreachable_destination() {
        // The export ops take already-loaded notes (so the caller can release
        // the store lock first), which leaves the write itself as their error
        // path.
        let s = store();
        seed(&s, "n1", "<p>x</p>");
        let notes = s.load_notes().unwrap();
        let nowhere = Path::new("/no/such/dir");
        assert!(export_notes_json(&notes, &nowhere.join("a.json"), &[]).is_err());
        assert!(
            export_notes_inlined(notes.clone(), nowhere, &nowhere.join("b.json"), &[]).is_err()
        );
        assert!(export_notes_bundle(notes, nowhere, &nowhere.join("bundle"), &[]).is_err());
        assert!(export_md_bundle(nowhere, &nowhere.join("md"), "# x", "n").is_err());
    }

    #[test]
    fn sync_collection_reports_the_failure() {
        let s = store_without_notes();
        assert!(collect_sync_push(&s).is_err());
        assert!(sync_status_synced(&s).is_err());
        assert!(commit_sync_result(&s, &[("n1".into(), 1)], &[], &[], &[], 1, 1).is_err());
    }

    #[test]
    fn folder_ops_report_a_missing_folders_table() {
        let s = store();
        s.conn.execute_batch("DROP TABLE folders;").unwrap();
        assert!(folder_create(&s, "f", "F", None).is_err());
        assert!(folder_rename(&s, "f", "F2").is_err());
        assert!(folder_move(&s, "f", None).is_err());
        assert!(folder_delete(&s, "f", "recursive").is_err());
        assert!(folders_reorder(&s, None, &["f".into()]).is_err());
        assert!(folder_set_icon(&s, "f", "i").is_err());
        assert!(folder_set_color(&s, "f", "c").is_err());
        assert!(folder_set_sort(&s, "f", "name").is_err());
        assert!(folder_chain_has_lock(&s, Some("f")).is_err());
        assert!(set_folder_locked(&s, &Dek::random(), "f", true).is_err());
    }

    #[test]
    fn vault_ops_report_a_missing_vault_table() {
        let s = store();
        s.conn.execute_batch("DROP TABLE vault;").unwrap();
        assert!(load_vault_record(&s).is_err());
        assert!(vault_setup(&s, "x").is_err());
        assert!(vault_change_passphrase(&s, "a", "b").is_err());
    }

    #[test]
    fn the_backfill_survives_a_missing_notes_table() {
        // It is best-effort by design: a broken database must not abort an
        // otherwise-successful vault unlock.
        let s = store_without_notes();
        backfill_protected_titles(&s, &Dek::random());
    }
}
