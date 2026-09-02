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
pub(crate) fn seal_content(dek: &Dek, note_id: &str, html: &str) -> String {
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
pub(crate) fn has_locked_ancestor_folder(store: &Store, note_id: &str) -> Result<bool, String> {
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
pub(crate) fn folder_chain_has_lock(
    store: &Store,
    starting_folder_id: Option<&str>,
) -> Result<bool, String> {
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
pub fn vault_unlock_passphrase(store: &Store, passphrase: &str) -> Result<Dek, String> {
    let record = load_vault_record(store)?;
    crate::vault::unlock_passphrase(&record, passphrase).map_err(String::from)
}

/// `vault_unlock_recovery`: same, but via the one-time recovery key (accepts
/// any formatting the user typed — separators and case are normalized).
pub fn vault_unlock_recovery(store: &Store, recovery: &str) -> Result<Dek, String> {
    let record = load_vault_record(store)?;
    crate::vault::unlock_recovery(&record, recovery).map_err(String::from)
}

/// `vault_change_passphrase`: unlock with `current`, re-wrap the SAME DEK
/// under `next`, and persist the updated record. The existing recovery key
/// keeps working — `rewrap_passphrase` never touches its wrapping.
///
/// Returns the (unchanged) DEK so the caller can re-arm the session: `current`
/// was just cryptographically re-verified, so that is safe rather than forcing
/// a redundant unlock.
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
pub fn export_notes_json(store: &Store, path: &Path, ids: &[String]) -> Result<(), String> {
    let notes = store.load_notes().map_err(|e| e.to_string())?;
    let json = crate::export::notes_to_json(&notes, ids).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

/// `export_notes_base64`: JSON export with every referenced image inlined as a
/// `data:` URL, so the file is self-contained.
pub fn export_notes_inlined(
    store: &Store,
    images_root: &Path,
    path: &Path,
    ids: &[String],
) -> Result<(), String> {
    let notes = store.load_notes().map_err(|e| e.to_string())?;
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
/// URLs (used for print/PDF). Looks in ALL notes, trashed ones included.
pub fn note_inlined_html(
    store: &Store,
    images_root: &Path,
    note_id: &str,
) -> Result<String, String> {
    let notes = store.load_all_notes().map_err(|e| e.to_string())?;
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
    store: &Store,
    images_root: &Path,
    dest: &Path,
    ids: &[String],
) -> Result<(), String> {
    let notes = store.load_notes().map_err(|e| e.to_string())?;
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
