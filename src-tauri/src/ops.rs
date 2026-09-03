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

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::profiles::{ContextEntry, Registry};
use crate::storage::{Note, SearchHit, Store};
use crate::vault::aead::Dek;
use crate::vault::kdf::KdfParams;
use crate::vault::state::VaultState;
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
    pub vault_exists: bool,
    pub vault_biometric: bool,
    /// The newest key generation this context's workspace vault has reached,
    /// as last pulled into that context's own database. 0 for a local context
    /// and for a server context that has never pulled the field.
    pub vault_generation: u32,
    /// Whether the workspace still owes this context's vault a key rotation
    /// (a member was removed and the key has not been rolled yet).
    pub vault_rotation_pending: bool,
    /// The user's role in the workspace as of the last pull; "" for local
    /// contexts.
    pub role: String,
    /// Open invitations whose vault code was lost in a rotation (owners
    /// only).
    pub invites_needing_code: u32,
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
    /// The workspace is waiting for its vault key to be rotated (a member was
    /// removed). Carried on the status so the sidebar badge does not have to
    /// re-list every context on every pull.
    pub vault_rotation_pending: bool,
}

/// A counter bumped every time the ACTIVE context's database is swapped out
/// (`commands::swap_store_to`).
///
/// The sync cycle reads the active context, then goes to the network without
/// holding any lock — and a context switch during that window would leave the
/// cycle writing workspace A's pull (and A's wrapped vault keys) into context
/// B's database, or uploading B's local vault record to A. Every step that
/// touches the store AFTER the network compares the epoch it captured at the
/// start of the cycle against the current one and bails out silently when
/// they differ; the next cycle simply redoes the work against the right
/// context.
///
/// An atomic rather than a mutex on purpose: it is read while the Store lock
/// is held, and adding a fourth lock to the Registry -> Store -> VaultState
/// order would be a needless deadlock risk.
#[derive(Default)]
pub struct SyncEpoch(std::sync::atomic::AtomicU64);

impl SyncEpoch {
    /// The epoch to capture at the start of a cycle.
    pub fn current(&self) -> u64 {
        self.0.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Invalidate every in-flight cycle — the active context just changed.
    pub fn bump(&self) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    /// Whether the active context changed since `captured` was taken.
    pub fn changed_since(&self, captured: u64) -> bool {
        self.current() != captured
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub exists: bool,
    pub unlocked: bool,
    pub biometric: bool,
    /// The workspace already held a vault that this device's own record did
    /// not create (meta `vault_conflict`, written by the sync migration hook).
    /// Surfaced as a warning — nothing is blocked, and it clears itself once
    /// an unlock proves the two are one vault.
    pub conflict: bool,
    /// Whether the recovery-key paths apply to this user at all — see
    /// [`vault_recovery_holder`]. An invited member holds no recovery key, so
    /// offering them "unlock with your recovery key" would be a dead end.
    pub recovery_holder: bool,
    /// The workspace rotated its key and parked this caller's new wrap under
    /// a one-time rotation code — the unlock flow asks for it.
    pub rotation_code: bool,
    /// This user holds the recovery key and some generation has no recovery
    /// wrap yet (somebody else rotated) — see [`generations_missing_recovery`].
    pub recovery_missing: bool,
    /// The workspace has rotated past every generation this device's ring
    /// holds, so every SEAL would be refused (see [`guard_seal_generation`]).
    /// The UI shows protected notes read-only while this is true rather than
    /// letting the user type into a note whose save cannot land.
    pub seal_outdated: bool,
    /// Whether the Security page offers "create a recovery key": this caller
    /// is a server-workspace owner who holds none yet and the vault is
    /// unlocked — see [`recovery_eligible`].
    pub recovery_eligible: bool,
    /// Whether the key the ring would seal with is the WORKSPACE's rather
    /// than this device's own. `false` on a conflicted device still sealing
    /// with its own vault's key, and `false` for a locked vault. Mirrors
    /// [`VaultStatusFlags::ring_is_workspace`] — see [`invite_wrap_allowed`]
    /// for the rule it feeds: a conflict alone never blocks minting an
    /// invite wrap, only a conflict paired with `!ring_is_workspace` does.
    pub ring_is_workspace: bool,
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

/// Whether the key the ring would seal with is one the WORKSPACE handed this
/// caller, rather than this device's own (conflicting) vault.
///
/// Generation NUMBERS alone cannot answer this: a device that set up its own
/// vault offline calls its key generation 1, and so does the workspace — two
/// different keys under one number. The cached `mine` entry's `dek_check` is
/// the only thing that settles it, and opening it is one cheap AEAD operation
/// (no Argon2: the wrap is not touched, only the check).
///
/// `false` when nothing is cached for that generation — an unknown key is
/// never assumed to be the workspace's.
pub fn ring_key_is_the_workspaces(
    entries: Option<&VaultEntries>,
    generation: u32,
    dek: &Dek,
) -> bool {
    entries.is_some_and(|e| {
        e.mine.iter().any(|m| {
            m.generation == generation
                && matches!(crate::vault::verify_dek(&m.record, dek), Ok(true))
        })
    })
}

/// Refuses to SEAL new content under `generation` while the workspace has
/// already rotated past it (meta `vault_generation`, written by every pull).
///
/// That gap means this device has not redeemed its rotation code yet. Sealing
/// under its stale newest generation would hand fresh plaintext to whoever
/// the rotation was meant to lock out — the removed member still knows that
/// key — and would add another note to the re-seal work list on top. Reading
/// and UNSEALING stay allowed: existing ciphertext is what the older
/// generations in the ring are for.
///
/// A local context (or a workspace that never rotated) has `vault_generation`
/// 0 or 1 and is never affected.
///
/// **Conflict-aware, but narrowly.** A device with meta `vault_conflict` may
/// be sealing with EITHER key: its own vault (the unlock fallback installs the
/// local record's generation 1 when the workspace entries do not open) or a
/// workspace one (that same fallback installs the workspace ring when they
/// do, and a redeemed rotation installs workspace generations outright). Only
/// the first deserves the exemption — comparing a private vault's numbering
/// against the workspace's would refuse every seal the moment the workspace
/// rotated, with advice ("unlock with your passphrase") that cannot help. A
/// workspace key is compared exactly as on any other device, because it is
/// exactly the key the rotation was meant to retire.
///
/// [`ring_key_is_the_workspaces`] tells the two apart by proof, not by
/// number. Notes sealed under the private key stay off the workspace key
/// because [`reseal_lagging_notes`] stands down on the same flag (C1).
///
/// `pub(crate)` so `mcp::StoreAccess::write_protected` can refuse BEFORE it
/// writes plaintext into the content column — it seals in two steps, and a
/// refusal after the first would leave the row plaintext with `protected = 1`.
pub(crate) fn guard_seal_generation(
    store: &Store,
    dek: &Dek,
    generation: u32,
) -> Result<(), String> {
    let conflicted = crate::migrate::get_meta_i64_opt(&store.conn, "vault_conflict")
        .map_err(|e| e.to_string())?
        .is_some();
    if conflicted
        && !ring_key_is_the_workspaces(cached_vault_entries(store)?.as_ref(), generation, dek)
    {
        return Ok(());
    }
    let server = crate::migrate::get_meta_i64(&store.conn, "vault_generation", 0);
    match server > i64::from(generation) {
        true => Err("vault: key generation outdated — unlock with your passphrase".to_string()),
        false => Ok(()),
    }
}

/// Whether this context is in the state [`guard_seal_generation`] refuses:
/// a workspace that has rotated past every generation this ring holds. Backs
/// `vault_status.seal_outdated`, which the UI uses to show protected notes
/// READ-ONLY rather than letting the user type into a note whose save would
/// be rejected.
///
/// `ring_is_workspace` carries [`ring_key_is_the_workspaces`]' answer for the
/// ring's newest generation, so the conflict exemption is exactly as narrow
/// here as it is in the guard.
///
/// A `ring_newest` of `None` (a locked vault) is deliberately NOT outdated:
/// protected notes already render as locked placeholders there, and claiming
/// "outdated key" on top would send the user after a rotation code when all
/// they need is to unlock.
pub fn seal_outdated(
    server_generation: i64,
    ring_newest: Option<u32>,
    is_server_context: bool,
    conflict: bool,
    ring_is_workspace: bool,
) -> bool {
    match ring_newest {
        _ if !is_server_context => false,
        // Conflicted AND sealing with this device's own vault: exempt.
        _ if conflict && !ring_is_workspace => false,
        None => false,
        Some(newest) => server_generation > i64::from(newest),
    }
}

/// Whether this device may mint an invite wrap (share or re-code) of the
/// ring's newest DEK. Refused when that DEK is not the workspace's current
/// key: on a conflicted device whose ring still seals with its own vault's
/// key, and on a device that has not redeemed the latest rotation — either
/// would hand the invitee a wrap that opens the wrong or a retired key.
pub fn invite_wrap_allowed(flags: &VaultStatusFlags, ring_newest: u32) -> Result<(), String> {
    if flags.conflict && !flags.ring_is_workspace {
        return Err("vault: resolve the vault conflict first".to_string());
    }
    if flags.server_generation > i64::from(ring_newest) {
        return Err("vault: redeem the rotation code first".to_string());
    }
    Ok(())
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
        .folder_chain_has_lock(starting_folder_id, None)
        .map_err(|e| e.to_string())
}

/// Like [`folder_chain_has_lock`], but treats `except` as always unlocked
/// regardless of its persisted `locked` flag. Used by
/// [`set_folder_locked`]'s `locked = false` branch to work out — BEFORE
/// `except` (the folder actually being unlocked) has its own flag flipped in
/// the database — whether a note would still have a genuinely locked
/// ancestor (some OTHER folder) once `except` itself becomes unlocked. Doing
/// this pre-flip, read-only, lets the caller validate every note it would
/// decrypt before committing anything, instead of flipping `except.locked`
/// first and discovering a missing generation partway through the per-note
/// loop.
fn folder_chain_has_lock_except(
    store: &Store,
    starting_folder_id: Option<&str>,
    except: &str,
) -> Result<bool, String> {
    store
        .folder_chain_has_lock(starting_folder_id, Some(except))
        .map_err(|e| e.to_string())
}

/// [`has_locked_ancestor_folder`]'s counterpart for
/// [`folder_chain_has_lock_except`]: true if `note_id`'s folder chain has a
/// locked ancestor OTHER than `except`.
fn has_locked_ancestor_folder_except(
    store: &Store,
    note_id: &str,
    except: &str,
) -> Result<bool, String> {
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
    folder_chain_has_lock_except(store, folder_id.as_deref(), except)
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
///
/// Refused outright while the workspace has rotated past `generation` (see
/// [`guard_seal_generation`]). Checked HERE, at the one choke point every
/// seal-on-transition goes through, so dragging a plaintext note into a
/// locked folder ([`reconcile_folder_move`], [`reconcile_reorder`]) cannot
/// quietly seal it under a key the rotation was meant to retire. Nothing is
/// written when it fires.
pub(crate) fn encrypt_note_in_place(
    store: &Store,
    id: &str,
    dek: &Dek,
    generation: u32,
) -> Result<(), String> {
    guard_seal_generation(store, dek, generation)?;
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
    // Ciphertext, its generation stamp, `protected` and the dirty flag are
    // ONE fact about the row. Written in a single transaction so a crash (or
    // a failing statement) can never leave the note sealed under `generation`
    // while `key_gen` still says something else — which `VaultState::dek_for`
    // would resolve to the wrong DEK and make the note permanently
    // unopenable. Same pattern as `Store::clear_note_dirty`.
    let tx = store
        .conn
        .unchecked_transaction()
        .map_err(|e| e.to_string())?;
    store
        .set_content_silent(id, &sealed)
        .map_err(|e| e.to_string())?;
    store.set_title(id, &title).map_err(|e| e.to_string())?;
    store
        .set_note_key_gen(id, Some(generation))
        .map_err(|e| e.to_string())?;
    store
        .set_note_protected(id, true)
        .map_err(|e| e.to_string())?;
    store
        .mark_note_dirty_if_syncing(id)
        .map_err(|e| e.to_string())?;
    crate::revisions::delete_revisions(&store.conn, id).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

/// Decrypt one currently-encrypted note in place: open its stored ciphertext
/// under the DEK it was ACTUALLY sealed with — `vault.dek_for(note_key_gen)`,
/// never just the ring's newest — write back the plaintext, and clear
/// `protected`. The inverse of [`encrypt_note_in_place`], shared by
/// `note_set_protected(false)` and `folder_set_locked(false)`.
///
/// A note sealed under an OLDER generation than the ring's newest (the normal
/// state mid-rotation) must still open under ITS OWN generation — reaching
/// for `vault.dek()` (newest) here, as an earlier revision of this function
/// did, would make such a note permanently un-unprotectable once the vault
/// rotates past its generation.
///
/// `Err("vault locked")` when the ring is empty, `Err("key generation not
/// available")` when it's unlocked but lacks this note's specific generation
/// — same two-error contract as [`open_note_content`].
///
/// Deliberately does NOT restore revision history (it was purged on the way in)
/// and does NOT re-mark the row dirty beyond what `set_content_silent` /
/// `set_note_protected` already do — matching the pre-refactor behavior of both
/// callers exactly.
fn decrypt_note_in_place(store: &Store, vault: &VaultState, id: &str) -> Result<(), String> {
    if !vault.is_unlocked() {
        return Err("vault locked".to_string());
    }
    let gen = store.note_key_gen(id).map_err(|e| e.to_string())?;
    let dek = vault
        .dek_for(gen)
        .ok_or_else(|| "key generation not available".to_string())?;
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
    // The note is plaintext again — clear the generation marker along with
    // `protected`, so it doesn't linger as stale metadata on an unsealed row.
    store
        .set_note_key_gen(id, None)
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
/// blob, foreign/mismatched key, or a generation not currently in `vault`'s
/// ring) is silently skipped rather than aborting the unlock, and nothing
/// here ever logs key or plaintext material. Only `title` is written —
/// `content` is read but never rewritten, preserving the
/// `content ciphertext ⟺ protected = 1` invariant.
pub fn backfill_protected_titles(store: &Store, vault: &VaultState) {
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
        let generation = match store.note_key_gen(&id) {
            Ok(g) => g,
            Err(_) => continue,
        };
        let Some(dek) = vault.dek_for(generation) else {
            continue; // that generation isn't unlocked — skip, never abort
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
/// longer exists). A protected note requires the vault to hold the DEK it was
/// actually sealed with: `Err("vault locked")` when the ring is empty,
/// `Err("key generation not available")` when the ring is unlocked but lacks
/// that particular generation (e.g. a key rotation the ring hasn't caught up
/// to). `None`/missing `key_gen` means generation 1 — see `VaultState::dek_for`.
pub fn open_note_content(store: &Store, vault: &VaultState, id: &str) -> Result<String, String> {
    let stored = match store.load_note_content(id).map_err(|e| e.to_string())? {
        Some(c) => c,
        None => return Ok(String::new()),
    };
    if !store.note_protected(id).map_err(|e| e.to_string())? {
        return Ok(stored);
    }
    if !vault.is_unlocked() {
        return Err("vault locked".to_string());
    }
    let gen = store.note_key_gen(id).map_err(|e| e.to_string())?;
    let unavailable = || "key generation not available".to_string();
    let dek = vault.dek_for(gen).ok_or_else(unavailable)?;
    match open_content(dek, id, &stored) {
        Ok(html) => Ok(html),
        // Belt and braces for a MIS-STAMPED note: a row whose `key_gen` does
        // not match the key its bytes were actually sealed with (an
        // interrupted pre-transaction re-seal from an older build, say) would
        // otherwise be unreadable forever, even though the right DEK is
        // sitting in the ring. Trying the other generations is cheap (AEAD,
        // not Argon2) and cannot succeed on the wrong key — the tag has to
        // verify. The ORIGINAL error stands when none of them opens it, so a
        // genuinely corrupt blob still reports itself as one.
        Err(e) => vault
            .generations()
            .into_iter()
            .filter(|g| Some(*g) != gen.or(Some(1)))
            .find_map(|g| open_content(vault.dek_for(Some(g))?, id, &stored).ok())
            .ok_or(e),
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
/// folder) is refused with `Err("vault locked")` when `vault` is `None`, is
/// stored as ciphertext under the given generation's DEK, and never
/// contributes a plaintext revision — any revisions recorded before the
/// transition are purged on every protected save.
///
/// `vault` is `Some((dek, generation))` rather than a bare `&Dek`: every
/// sealing write records `generation` into `notes.key_gen` (see
/// `Store::save_note`'s `ON CONFLICT` clause, which otherwise defaults it to
/// the incoming `Note`'s — usually `None` — value), so a later open picks the
/// SAME DEK back out of the ring rather than always reaching for the newest.
pub fn save_note(store: &Store, vault: Option<(&Dek, u32)>, note: &Note) -> Result<(), String> {
    let title = crate::storage::note_preview(&note.content);
    let protected = store
        .is_effectively_protected(&note.id)
        .map_err(|e| e.to_string())?;
    if protected {
        let (dek, generation) = vault.ok_or_else(|| "vault locked".to_string())?;
        guard_seal_generation(store, dek, generation)?;
        let mut sealed = note.clone();
        sealed.content = seal_content(dek, &note.id, &note.content);
        // The `Note` coming in from the frontend (or built via
        // `..Default::default()`) carries `key_gen: None`, and
        // `store.save_note`'s `ON CONFLICT` clause would write that NULL —
        // which `VaultState::dek_for` resolves to generation 1 and could make
        // the note permanently unopenable when it was actually sealed under a
        // later generation. Stamp the clone BEFORE it is written, so the
        // ciphertext and its generation land in the SAME statement and no
        // crash can separate them.
        sealed.key_gen = Some(generation);
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

/// Core reconciliation logic behind `notes_set_folder`: `vault` is `None` to
/// represent a locked vault, `Some((dek, generation))` unlocked — sealing
/// always uses the newest ring generation, same as [`save_note`].
///
/// Moves only ever ADD protection, never remove it:
/// - Already-encrypted notes stay encrypted regardless of destination — the
///   safe direction; moving an encrypted note out of a locked folder does
///   NOT auto-decrypt it (the user can explicitly unprotect it).
/// - A currently-plaintext note moving into a location with a locked
///   ancestor folder must become encrypted. That check — and the
///   `vault.is_some()` requirement it implies — happens BEFORE the move is
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
    vault: Option<(&Dek, u32)>,
) -> Result<(), String> {
    let already_protected = store.note_protected(id).map_err(|e| e.to_string())?;
    let needs_encryption = !already_protected && folder_chain_has_lock(store, folder_id)?;

    if needs_encryption {
        let (dek, generation) = vault.ok_or_else(|| "vault locked".to_string())?;
        // BEFORE the move. `Store::set_folder` autocommits (and marks the row
        // dirty in a syncing context), so refusing after it would leave a
        // PLAINTEXT note sitting inside the locked subtree with
        // `protected = 0` — and queued to be pushed that way. Same
        // refuse-before-mutating order as `set_note_protected` /
        // `set_folder_locked`.
        guard_seal_generation(store, dek, generation)?;
        store.set_folder(id, folder_id).map_err(|e| e.to_string())?;
        encrypt_note_in_place(store, id, dek, generation)?;
    } else {
        store.set_folder(id, folder_id).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Core reconciliation behind `notes_reorder`, sharing
/// [`reconcile_folder_move`]'s convention: `vault` is `None` for a locked
/// vault, `Some((dek, generation))` unlocked.
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
    vault: Option<(&Dek, u32)>,
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
    if !to_encrypt.is_empty() && vault.is_none() {
        return Err("vault locked".to_string());
    }
    // Likewise for a ring the workspace has rotated past: `reorder_notes`
    // autocommits, so a refusal after it would leave every note in
    // `to_encrypt` inside the locked subtree, plaintext and dirty.
    if let Some((dek, generation)) = vault.filter(|_| !to_encrypt.is_empty()) {
        guard_seal_generation(store, dek, generation)?;
    }

    store
        .reorder_notes(folder_id, ids)
        .map_err(|e| e.to_string())?;
    if let Some((dek, generation)) = vault {
        for id in to_encrypt {
            encrypt_note_in_place(store, id, dek, generation)?;
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
    // [`vault_exists`], not just the local record: a device that joined a
    // workspace vault through an invitation for a generation > 1 mirrors no
    // record at all, and letting it "set up" here would mint a SECOND,
    // incompatible DEK for a context whose notes are sealed under the
    // workspace's.
    if vault_exists(store)? {
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
/// check (a key from another context), and refuses to guess when nothing can
/// verify it: such a vault must first be unlocked with its passphrase or
/// recovery key once, which writes the check (`ensure_dek_check`).
///
/// Verified against the source that actually covers `generation`:
///
/// - the workspace's cached `mine` entry for that generation, when there is
///   one. This is the only source once the key has rotated — the local
///   record only ever mirrors generation 1 — and it is why the keychain item
///   carries its generation alongside the DEK.
/// - otherwise, and only for generation 1, this device's own record.
///
/// A generation with neither is unverifiable, never "accepted anyway": an
/// unverified DEK installed into the ring would go on to SEAL new content.
pub fn verify_dek_for_store(store: &Store, generation: u32, dek: &Dek) -> Result<(), String> {
    let unverifiable = || {
        "vault: unlock with your passphrase once to finish upgrading this vault, then re-enable biometric unlock"
            .to_string()
    };
    let judge = |record: &VaultRecord| match crate::vault::verify_dek(record, dek) {
        Ok(true) => Ok(()),
        Ok(false) => Err(unverifiable()),
        Err(_) => Err("vault: biometric key belongs to a different context".to_string()),
    };
    if let Some(entry) = cached_vault_entries(store)?
        .and_then(|e| e.mine.into_iter().find(|e| e.generation == generation))
    {
        return judge(&entry.record);
    }
    match generation {
        1 => judge(&load_vault_record(store)?),
        _ => Err(unverifiable()),
    }
}

/// Runs [`verify_dek_for_store`] over a whole biometric-unlock ring,
/// partitioning it into the generations that actually open this vault and
/// the ones that don't (with their error text). Preserves `ring`'s order —
/// callers pass it ascending (`VaultState::snapshot`, `biometric::load_ring`),
/// so the verified half comes back ascending too.
///
/// Shared by both biometric commands: `vault_biometric_enable` writes only
/// the verified half to the keychain (nothing unverified is ever stored),
/// and `vault_unlock_biometric` installs only the verified half into
/// `VaultState`, logging the rest instead of failing the whole unlock.
#[allow(clippy::type_complexity)]
pub fn verify_ring_for_store(
    store: &Store,
    ring: &[(u32, Dek)],
) -> (Vec<(u32, Dek)>, Vec<(u32, String)>) {
    let mut verified = Vec::new();
    let mut rejected = Vec::new();
    for (generation, dek) in ring {
        match verify_dek_for_store(store, *generation, dek) {
            Ok(()) => verified.push((*generation, dek.clone())),
            Err(e) => rejected.push((*generation, e)),
        }
    }
    (verified, rejected)
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

// ---------------------------------------------------------------------------
// Workspace vault keys
//
// For a server-bound context the wrapped vault keys live on the workspace,
// one entry per key generation: `mine` is the caller's own passphrase wrap,
// `recovery` the workspace-wide recovery wrap. `apply_vault_keys` caches
// whatever the server sent on every pull (see `commit_sync_result`);
// everything below turns that cache into unlocked DEKs, rewraps it on a
// passphrase change, and turns a local-only vault into the upload that seeds
// the workspace.
//
// Only wraps and the sealed `dek_check` magic ever travel — the DEK itself
// stays on the device, and the server can open neither.
// ---------------------------------------------------------------------------

/// One `vaultKeys.mine[]` element: exactly what the server sends and exactly
/// the body `PUT …/vault/keys/me` expects back.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MyEntryWire {
    pub generation: u32,
    pub kdf_params: KdfParams,
    pub dek_wrapped: String,
    pub dek_check: String,
}

/// One `vaultKeys.recovery[]` element — the workspace-wide recovery wrap for
/// a generation. `dek_check` is optional: entries written before the check
/// existed simply omit it.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryEntryWire {
    pub generation: u32,
    pub recovery_salt: String,
    pub dek_wrapped_recovery: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dek_check: Option<String>,
}

/// The cached `{"mine":[…],"recovery":[…],"rotation":[…]}` blob as it sits in
/// `vault.entries` — the server's wire shape, stored verbatim.
///
/// `rotation` carries the caller's own rotation wraps: after a member was
/// removed the rotating owner wraps the NEW generation for every remaining
/// member under a one-time rotation code, and the member turns that into a
/// wrap of their own (`vault_rotation_redeem`). Every list is
/// `#[serde(default)]`, so a cache written before rotation wraps existed
/// still parses.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct VaultEntriesWire {
    #[serde(default)]
    mine: Vec<MyEntryWire>,
    #[serde(default)]
    recovery: Vec<RecoveryEntryWire>,
    #[serde(default)]
    rotation: Vec<MyEntryWire>,
}

/// A parsed [`MyEntryWire`]: the passphrase wrap for one generation, shaped
/// as a [`VaultRecord`] so the existing crypto works on it unchanged. The
/// recovery half is empty here — it lives in [`RecoveryEntry`].
#[derive(Clone)]
pub struct MyEntry {
    pub generation: u32,
    pub record: VaultRecord,
}

/// A parsed [`RecoveryEntryWire`].
#[derive(Clone)]
pub struct RecoveryEntry {
    pub generation: u32,
    pub recovery_salt: [u8; 16],
    pub dek_wrapped_recovery: Vec<u8>,
    pub dek_check: Option<Vec<u8>>,
}

/// Every wrapped key the workspace holds for this caller, by generation.
#[derive(Clone, Default)]
pub struct VaultEntries {
    pub mine: Vec<MyEntry>,
    pub recovery: Vec<RecoveryEntry>,
    /// Wraps waiting for a one-time ROTATION code (same wire shape as
    /// `mine`, but the KEK derives from the code, not from a passphrase).
    /// Emptied generation by generation as [`merge_my_entry`] installs the
    /// member's own wrap.
    pub rotation: Vec<MyEntry>,
}

/// Base64 that treats an empty string as "field absent" — a server that
/// sends `""` for a check it doesn't have must not produce a check blob that
/// can never be opened.
fn decode_opt_b64(s: &str) -> Result<Option<Vec<u8>>, String> {
    if s.is_empty() {
        return Ok(None);
    }
    STANDARD.decode(s).map(Some).map_err(|e| e.to_string())
}

impl From<&MyEntry> for MyEntryWire {
    fn from(e: &MyEntry) -> Self {
        MyEntryWire {
            generation: e.generation,
            kdf_params: e.record.kdf_params.clone(),
            dek_wrapped: STANDARD.encode(&e.record.dek_wrapped_pass),
            dek_check: e
                .record
                .dek_check
                .as_deref()
                .map(|c| STANDARD.encode(c))
                .unwrap_or_default(),
        }
    }
}

impl TryFrom<MyEntryWire> for MyEntry {
    type Error = String;
    fn try_from(w: MyEntryWire) -> Result<Self, String> {
        // These parameters come straight off the wire and go straight into
        // Argon2. An absurd `m_cost` would have the unlock try to allocate
        // gigabytes; a zero one is simply invalid. Refuse the entry instead
        // — `cached_vault_entries` degrades that to "no usable cache" and
        // falls back to the local record rather than failing hard.
        if !w.kdf_params.is_within_limits() {
            return Err("vault: entry has out-of-range key-derivation parameters".to_string());
        }
        Ok(MyEntry {
            generation: w.generation,
            record: VaultRecord {
                kdf_params: w.kdf_params,
                dek_wrapped_pass: STANDARD.decode(&w.dek_wrapped).map_err(|e| e.to_string())?,
                recovery_salt: [0u8; 16],
                dek_wrapped_recovery: Vec::new(),
                dek_check: decode_opt_b64(&w.dek_check)?,
            },
        })
    }
}

impl From<&RecoveryEntry> for RecoveryEntryWire {
    fn from(e: &RecoveryEntry) -> Self {
        RecoveryEntryWire {
            generation: e.generation,
            recovery_salt: STANDARD.encode(e.recovery_salt),
            dek_wrapped_recovery: STANDARD.encode(&e.dek_wrapped_recovery),
            dek_check: e.dek_check.as_deref().map(|c| STANDARD.encode(c)),
        }
    }
}

impl TryFrom<RecoveryEntryWire> for RecoveryEntry {
    type Error = String;
    fn try_from(w: RecoveryEntryWire) -> Result<Self, String> {
        let recovery_salt: [u8; 16] = STANDARD
            .decode(&w.recovery_salt)
            .map_err(|e| e.to_string())?
            .try_into()
            .map_err(|_| "vault: recovery salt must be 16 bytes".to_string())?;
        Ok(RecoveryEntry {
            generation: w.generation,
            recovery_salt,
            dek_wrapped_recovery: STANDARD
                .decode(&w.dek_wrapped_recovery)
                .map_err(|e| e.to_string())?,
            dek_check: decode_opt_b64(w.dek_check.as_deref().unwrap_or_default())?,
        })
    }
}

impl VaultEntries {
    /// The wire shape `apply_vault_keys` caches — byte fields base64-encoded
    /// with the same `STANDARD` engine `VaultRecord::to_json` uses.
    pub fn to_json(&self) -> String {
        let wire = VaultEntriesWire {
            mine: self.mine.iter().map(MyEntryWire::from).collect(),
            recovery: self.recovery.iter().map(RecoveryEntryWire::from).collect(),
            rotation: self.rotation.iter().map(MyEntryWire::from).collect(),
        };
        serde_json::to_string(&wire).expect("VaultEntriesWire has no non-serializable fields")
    }

    /// Parses the cached blob. Any malformed JSON, base64 or field length is
    /// an `Err` — callers treat that as "no usable cache" rather than
    /// failing the unlock outright (see [`cached_vault_entries`]).
    pub fn from_json(s: &str) -> Result<Self, String> {
        let wire: VaultEntriesWire = serde_json::from_str(s).map_err(|e| e.to_string())?;
        Ok(VaultEntries {
            mine: wire
                .mine
                .into_iter()
                .map(MyEntry::try_from)
                .collect::<Result<Vec<_>, _>>()?,
            recovery: wire
                .recovery
                .into_iter()
                .map(RecoveryEntry::try_from)
                .collect::<Result<Vec<_>, _>>()?,
            rotation: wire
                .rotation
                .into_iter()
                .map(MyEntry::try_from)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.mine.is_empty() && self.recovery.is_empty() && self.rotation.is_empty()
    }
}

/// How many generations one unlock attempt will derive a KEK for. Each one is
/// a deliberately expensive Argon2 pass, so an unbounded list — the server
/// decides how long it is — would turn a single unlock into minutes of work.
/// Far above any realistic rotation count; the NEWEST generations are the
/// ones kept, since those are what new content seals under.
const MAX_UNLOCK_GENERATIONS: usize = 32;

/// The newest [`MAX_UNLOCK_GENERATIONS`] of `items`, back in ascending order.
/// Sorts rather than trusting the server's ordering.
fn newest_generations<T>(items: &[T], generation: impl Fn(&T) -> u32) -> Vec<&T> {
    let mut refs: Vec<&T> = items.iter().collect();
    refs.sort_by_key(|e| std::cmp::Reverse(generation(e)));
    refs.truncate(MAX_UNLOCK_GENERATIONS);
    refs.sort_by_key(|e| generation(e));
    refs
}

/// Unwraps every `mine` entry `passphrase` opens, verifying each entry's DEK
/// check before accepting it (an entry that carries no check is accepted —
/// the AEAD unwrap already authenticated it). Returns one `(generation,
/// DEK)` per entry that opened, ascending; `Err("wrong passphrase")` when
/// none did. At most [`MAX_UNLOCK_GENERATIONS`] entries are tried.
pub fn unlock_entries_with_passphrase(
    entries: &VaultEntries,
    passphrase: &str,
) -> Result<Vec<(u32, Dek)>, String> {
    let mut out = Vec::new();
    for e in newest_generations(&entries.mine, |e| e.generation) {
        if let Ok(dek) = crate::vault::unlock_passphrase(&e.record, passphrase) {
            if crate::vault::verify_dek(&e.record, &dek).is_ok() {
                out.push((e.generation, dek));
            }
        }
    }
    if out.is_empty() {
        return Err("wrong passphrase".to_string());
    }
    Ok(out)
}

/// [`unlock_entries_with_passphrase`] over the `recovery` half: each entry is
/// reshaped into a recovery-only [`VaultRecord`] and opened with the (freely
/// formatted) recovery key.
pub fn unlock_entries_with_recovery(
    entries: &VaultEntries,
    recovery: &str,
) -> Result<Vec<(u32, Dek)>, String> {
    let mut out = Vec::new();
    for e in newest_generations(&entries.recovery, |e| e.generation) {
        let rec = crate::vault::recovery_only_record(
            e.recovery_salt,
            e.dek_wrapped_recovery.clone(),
            e.dek_check.clone(),
        );
        if let Ok(dek) = crate::vault::unlock_recovery(&rec, recovery) {
            if crate::vault::verify_dek(&rec, &dek).is_ok() {
                out.push((e.generation, dek));
            }
        }
    }
    if out.is_empty() {
        return Err("wrong recovery key".to_string());
    }
    Ok(out)
}

/// The body of `POST …/vault`: generation 1's passphrase wrap plus the
/// workspace recovery wrap.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupPayload {
    pub kdf_params: KdfParams,
    pub dek_wrapped: String,
    pub dek_check: String,
    pub recovery: RecoveryPayload,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryPayload {
    pub recovery_salt: String,
    pub dek_wrapped_recovery: String,
    pub dek_check: String,
}

/// An existing local record as the generation-1 upload body — the exact
/// record this device already stores, so the vault the workspace gets is the
/// one whose DEK already sealed this device's notes.
///
/// `Err` when the record predates `dek_check`: without it the server would
/// hold entries no unlock path could verify. Such a record self-heals on its
/// next passphrase/recovery unlock (see [`ensure_dek_check`]) and is simply
/// skipped until then.
pub fn migration_payload(rec: &VaultRecord) -> Result<SetupPayload, String> {
    let check = rec
        .dek_check
        .as_deref()
        .ok_or_else(|| "vault: record has no DEK check yet".to_string())?;
    Ok(SetupPayload {
        kdf_params: rec.kdf_params.clone(),
        dek_wrapped: STANDARD.encode(&rec.dek_wrapped_pass),
        dek_check: STANDARD.encode(check),
        recovery: RecoveryPayload {
            recovery_salt: STANDARD.encode(rec.recovery_salt),
            dek_wrapped_recovery: STANDARD.encode(&rec.dek_wrapped_recovery),
            dek_check: STANDARD.encode(check),
        },
    })
}

/// A brand-new vault as an upload body, without persisting anything.
///
/// The `vault_setup` command deliberately does NOT use this: it runs the
/// local [`vault_setup`] first and uploads
/// [`migration_payload`] of the record it just stored, so the workspace can
/// never end up holding a different vault than this device does. Kept (and
/// tested) as the payload-side counterpart of [`migration_payload`] for
/// callers that need the body without a store.
#[allow(dead_code)]
pub fn vault_setup_payload(passphrase: &str) -> Result<(SetupPayload, Vec<String>, Dek), String> {
    let (record, recovery_key, dek) = crate::vault::setup(passphrase).map_err(String::from)?;
    let groups = recovery_key.as_str().split('-').map(String::from).collect();
    Ok((migration_payload(&record)?, groups, dek))
}

/// The cached server entries, or `None` when nothing is cached, the cache is
/// empty, or it cannot be parsed. An unparsable cache is deliberately not an
/// error: the local record is still a valid way in, and failing every unlock
/// over a corrupt cache would lock the user out of their own notes.
pub fn cached_vault_entries(store: &Store) -> Result<Option<VaultEntries>, String> {
    let Some(json) = store.vault_entries().map_err(|e| e.to_string())? else {
        return Ok(None);
    };
    match VaultEntries::from_json(&json) {
        Ok(entries) => Ok(Some(entries).filter(|e| !e.is_empty())),
        Err(e) => {
            // The cache is opaque server bytes (`apply_vault_keys` stores
            // `vaultKeys` verbatim), so a naming/encoding disagreement with
            // the server would otherwise degrade silently to "no vault keys"
            // — and dead-end a new device. Never log the payload itself: it
            // is wrapped key material.
            eprintln!("vault entries cache unusable ({e}); falling back to the local record");
            Ok(None)
        }
    }
}

/// Everything the unlock paths read from the store, in one lock scope — the
/// local record (if this device has one) and the server's cached entries.
pub struct VaultUnlockInputs {
    pub record: Option<VaultRecord>,
    pub entries: Option<VaultEntries>,
}

pub fn load_vault_unlock_inputs(store: &Store) -> Result<VaultUnlockInputs, String> {
    let record = match store.vault_record().map_err(|e| e.to_string())? {
        Some(json) => Some(VaultRecord::from_json(&json).map_err(String::from)?),
        None => None,
    };
    Ok(VaultUnlockInputs {
        record,
        entries: cached_vault_entries(store)?,
    })
}

/// Backs `vault_status.exists`: a vault exists for this context if this
/// device stored a record OR the workspace handed back a wrapped key for
/// this caller. Without the second half a freshly synced device would be
/// offered "set up a vault" — which would generate a second, incompatible
/// DEK — instead of "unlock".
pub fn vault_exists(store: &Store) -> Result<bool, String> {
    if store.vault_record().map_err(|e| e.to_string())?.is_some() {
        return Ok(true);
    }
    Ok(cached_vault_entries(store)?.is_some_and(|e| !e.mine.is_empty()))
}

/// Whether "set up a vault" on this store would really mean "unlock the one
/// the workspace already has": no local record, but a wrapped key cached for
/// this caller. [`vault_setup`] refuses both cases with the same generic
/// error; the command layer uses this to say the actionable thing instead.
pub fn server_vault_needs_unlock(store: &Store) -> Result<bool, String> {
    if store.vault_record().map_err(|e| e.to_string())?.is_some() {
        return Ok(false);
    }
    // No record, so "a vault exists" can only mean the workspace's — one
    // definition of existence, in [`vault_exists`], rather than two copies
    // that could drift apart.
    vault_exists(store)
}

/// The generation-1 entries as a local [`VaultRecord`], merging the
/// passphrase wrap with the matching recovery wrap so the mirrored record
/// opens both ways.
fn mirrored_record(entries: &VaultEntries) -> Option<VaultRecord> {
    let mine = entries.mine.iter().find(|e| e.generation == 1)?;
    let recovery = entries.recovery.iter().find(|e| e.generation == 1);
    Some(VaultRecord {
        kdf_params: mine.record.kdf_params.clone(),
        dek_wrapped_pass: mine.record.dek_wrapped_pass.clone(),
        recovery_salt: recovery.map(|r| r.recovery_salt).unwrap_or([0u8; 16]),
        dek_wrapped_recovery: recovery
            .map(|r| r.dek_wrapped_recovery.clone())
            .unwrap_or_default(),
        dek_check: mine
            .record
            .dek_check
            .clone()
            .or_else(|| recovery.and_then(|r| r.dek_check.clone())),
    })
}

/// Which secret an unlock path is using. Lets the two unlock commands share
/// one body (see `commands::unlock_vault_with`) instead of duplicating it.
#[derive(Clone, Copy)]
pub enum VaultSecret<'a> {
    Passphrase(&'a str),
    Recovery(&'a str),
}

impl VaultSecret<'_> {
    /// Opens a whole [`VaultRecord`] with this secret.
    pub fn open(&self, record: &VaultRecord) -> Result<Dek, String> {
        match self {
            VaultSecret::Passphrase(p) => vault_unlock_passphrase(record, p),
            VaultSecret::Recovery(r) => vault_unlock_recovery(record, r),
        }
    }

    /// Opens every server entry this secret can, via the matching half of
    /// the cache.
    pub fn open_entries(&self, entries: &VaultEntries) -> Result<Vec<(u32, Dek)>, String> {
        match self {
            VaultSecret::Passphrase(p) => unlock_entries_with_passphrase(entries, p),
            VaultSecret::Recovery(r) => unlock_entries_with_recovery(entries, r),
        }
    }

    /// Whether the cache carries anything this secret could open at all.
    pub fn entries_usable(&self, entries: &VaultEntries) -> bool {
        match self {
            VaultSecret::Passphrase(_) => !entries.mine.is_empty(),
            VaultSecret::Recovery(_) => !entries.recovery.is_empty(),
        }
    }
}

/// What an unlock-from-entries may install, and whether the local record and
/// the workspace turned out to be the same vault.
pub struct VaultUnlockPlan {
    /// The generations to install into the ring.
    pub install: Vec<(u32, Dek)>,
    /// `true` when this device's record and the workspace vault are provably
    /// one vault (or this device has no record yet). Only then may the store
    /// be reconciled — see [`apply_entry_unlock`].
    pub reconciled: bool,
}

/// "Same DEK ⇒ the same `dek_check` opens both": does `server_dek` belong to
/// `rec`?
///
/// A record written before `dek_check` existed has nothing to verify
/// against, so the workspace DEK is compared against the one the same secret
/// unwraps from the record itself. That costs an extra KDF derivation, but
/// only on records that predate the check.
fn same_vault(rec: &VaultRecord, server_dek: &Dek, secret: &VaultSecret<'_>) -> bool {
    match crate::vault::verify_dek(rec, server_dek) {
        Ok(true) => true,
        Err(_) => false,
        Ok(false) => secret
            .open(rec)
            .is_ok_and(|local| local.expose() == server_dek.expose()),
    }
}

/// Decides what an unlock-from-entries installs, and whether the store may
/// be reconciled afterwards.
///
/// Possession of the WORKSPACE DEK says nothing about the LOCAL record's
/// DEK. A vault set up offline on this device and a vault set up on the
/// workspace from another device can share a passphrase and still wrap two
/// different DEKs. Installing the workspace ring on such a device would
/// leave this device's own protected notes undecryptable while
/// `vault_status.unlocked` reports `true`, and healing `dek_check` from the
/// foreign DEK would poison `verify_dek_for_store` and break biometric
/// unlock permanently. So:
///
/// - The workspace entries do not open with this secret at all, but the
///   local record does: a conflicted device unlocking its own vault —
///   install generation 1 from the record, reconcile nothing. (If neither
///   opens, the entries' error is returned.)
/// - No local record: nothing to disagree with — install the workspace ring
///   and reconcile.
/// - The workspace generation-1 DEK provably opens the local record: one
///   vault — install the whole ring and reconcile.
/// - The workspace ring has NO generation 1 (this member joined at a later
///   generation): nothing to compare against — install the workspace ring,
///   reconcile nothing.
/// - Otherwise the two are DIFFERENT vaults. Prefer the local one when the
///   same secret opens it (generation 1 only — never mixed with the
///   workspace ring), so this device keeps reading its own notes; fall back
///   to the workspace ring when it does not. Either way nothing is
///   reconciled: `vault_conflict` stays set for Task 13 to surface, the
///   local record's check is left alone, and `vault_migrated` is NOT
///   claimed, so the sync hook keeps re-marking the conflict.
pub fn plan_entry_unlock(
    local: Option<&VaultRecord>,
    entries: &VaultEntries,
    secret: &VaultSecret<'_>,
) -> Result<VaultUnlockPlan, String> {
    let opened = match secret.open_entries(entries) {
        Ok(opened) => opened,
        Err(e) => {
            // The workspace vault does not open with this secret at all. If
            // this device has a record of its OWN that does, it is simply a
            // conflicted device unlocking its own vault — the workspace
            // entries must not turn that into a lockout.
            let dek = local.and_then(|rec| secret.open(rec).ok()).ok_or(e)?;
            return Ok(VaultUnlockPlan {
                install: vec![(1, dek)],
                reconciled: false,
            });
        }
    };
    let Some(rec) = local else {
        return Ok(VaultUnlockPlan {
            install: opened,
            reconciled: true,
        });
    };
    let Some((_, gen1)) = opened.iter().find(|(g, _)| *g == 1) else {
        // The workspace ring has no generation 1 at all — this member was
        // invited at a later generation, so there is nothing here to compare
        // the local record against. That is UNKNOWN, not a mismatch: prefer
        // the workspace ring (what this context's notes are actually sealed
        // under) and reconcile nothing, rather than falling back to a local
        // generation 1 that the workspace never had. The two rings are never
        // merged — mixing two vaults' generations is exactly the state
        // `vault_conflict` exists to prevent.
        return Ok(VaultUnlockPlan {
            install: opened,
            reconciled: false,
        });
    };
    if same_vault(rec, gen1, secret) {
        return Ok(VaultUnlockPlan {
            install: opened,
            reconciled: true,
        });
    }
    Ok(match secret.open(rec) {
        Ok(local_dek) => VaultUnlockPlan {
            install: vec![(1, local_dek)],
            reconciled: false,
        },
        Err(_) => VaultUnlockPlan {
            install: opened,
            reconciled: false,
        },
    })
}

/// Everything an unlock-from-entries settles in the store afterwards — but
/// only once [`plan_entry_unlock`] proved the local record and the workspace
/// hold the same vault:
///
/// - No local record (a new device): mirror generation 1 into one, so
///   `vault_status`, biometric enrolment and [`ensure_dek_check`] — all of
///   which read the local record — keep working.
/// - A local record unlocked with a PASSPHRASE: rewrap it under that
///   passphrase. A passphrase change made on another device rewraps the
///   workspace entries but cannot reach this device's own record, so the OLD
///   passphrase would keep opening the vault here — exactly what the change
///   was meant to revoke. The rewrap re-establishes the DEK check on the way
///   (see `vault::rewrap_passphrase`), so it subsumes [`ensure_dek_check`].
/// - A local record unlocked with the RECOVERY key: only self-heal its DEK
///   check. A recovery key is not a passphrase and must not become one.
///
/// Safe in both cases only because `plan.reconciled` is true: the
/// generation-1 DEK was just proved to be this record's own. A conflicted
/// plan writes nothing at all.
pub fn apply_entry_unlock(
    store: &Store,
    local: Option<&VaultRecord>,
    entries: &VaultEntries,
    plan: &VaultUnlockPlan,
    secret: &VaultSecret<'_>,
) -> Result<(), String> {
    if !plan.reconciled {
        return Ok(());
    }
    match local {
        None => {
            if let Some(mirrored) = mirrored_record(entries) {
                store
                    .set_vault_record(&mirrored.to_json())
                    .map_err(|e| e.to_string())?;
            }
        }
        Some(rec) => {
            if let Some((_, dek)) = plan.install.iter().find(|(g, _)| *g == 1) {
                match secret {
                    VaultSecret::Passphrase(p) => store
                        .set_vault_record(&crate::vault::rewrap_passphrase(rec, dek, p).to_json())
                        .map_err(|e| e.to_string())?,
                    VaultSecret::Recovery(_) => ensure_dek_check(store, rec, dek),
                }
            }
        }
    }
    crate::migrate::delete_meta(&store.conn, "vault_conflict").map_err(|e| e.to_string())?;
    crate::migrate::set_meta_i64(&store.conn, "vault_migrated", 1).map_err(|e| e.to_string())
}

/// The uploads to send BACK after a passphrase change failed partway through
/// its PUT loop: the still-cached OLD wrap of every generation whose new wrap
/// already landed, ascending.
///
/// Without this a workspace with N generations can end up split — some
/// generations wrapped under the new passphrase, the rest under the old — and
/// no single passphrase would open the whole ring. `uploaded` lists the
/// generations the loop got through before it failed; a generation the cache
/// no longer describes is skipped rather than guessed at.
///
/// Best-effort by contract: the caller sends these and returns the ORIGINAL
/// error either way, so a revert that also fails does not mask why the change
/// was refused in the first place.
pub fn rewrap_revert_uploads(entries: &VaultEntries, uploaded: &[u32]) -> Vec<MyEntryWire> {
    let mut out: Vec<MyEntryWire> = entries
        .mine
        .iter()
        .filter(|e| uploaded.contains(&e.generation))
        .map(MyEntryWire::from)
        .collect();
    out.sort_by_key(|e| e.generation);
    out
}

/// The result of rewrapping a server context's vault under a new
/// passphrase: what to upload, and what to persist once every upload landed.
pub struct VaultRewrap {
    /// The local record under the new passphrase (`None` if this device has
    /// none — it unlocks purely from the cache).
    pub record: Option<VaultRecord>,
    /// The rewrapped entry cache, recovery half untouched.
    pub entries: VaultEntries,
    /// One `PUT …/vault/keys/me` body per rewrapped generation.
    pub uploads: Vec<MyEntryWire>,
    /// The (unchanged) DEKs, to re-arm the ring afterwards.
    pub deks: Vec<(u32, Dek)>,
}

/// Rewraps every generation `current` opens under `next`, in memory only —
/// no store writes and no network. The caller uploads [`VaultRewrap::uploads`]
/// first and persists the rest only once every upload succeeded, so a failed
/// PUT can never leave this device on a passphrase the workspace never
/// learned about.
///
/// The DEKs themselves are untouched, so existing ciphertext (and the
/// recovery key) keeps working. A generation `current` does not open is left
/// exactly as cached — rewrapping it would need a DEK we don't have.
pub fn rewrap_for_server(
    local: Option<&VaultRecord>,
    entries: &VaultEntries,
    current: &str,
    next: &str,
) -> Result<VaultRewrap, String> {
    let deks = unlock_entries_with_passphrase(entries, current)?;

    let mut mine = Vec::with_capacity(entries.mine.len());
    let mut uploads = Vec::new();
    for e in &entries.mine {
        match deks.iter().find(|(g, _)| *g == e.generation) {
            Some((_, dek)) => {
                let entry = MyEntry {
                    generation: e.generation,
                    record: crate::vault::rewrap_passphrase(&e.record, dek, next),
                };
                uploads.push(MyEntryWire::from(&entry));
                mine.push(entry);
            }
            None => mine.push(e.clone()),
        }
    }

    let record = match local {
        None => None,
        Some(rec) => {
            // Reuse the generation-1 DEK when it provably belongs to this
            // record (the usual case: the record mirrors generation 1),
            // rather than paying a second Argon2 derivation for it.
            let reuse = deks
                .iter()
                .find(|(g, _)| *g == 1)
                .map(|(_, d)| d.clone())
                .filter(|d| matches!(crate::vault::verify_dek(rec, d), Ok(true)));
            let dek = match reuse {
                Some(d) => d,
                None => crate::vault::unlock_passphrase(rec, current).map_err(String::from)?,
            };
            Some(crate::vault::rewrap_passphrase(rec, &dek, next))
        }
    };

    Ok(VaultRewrap {
        record,
        entries: VaultEntries {
            mine,
            recovery: entries.recovery.clone(),
            // Rotation wraps derive their KEK from the one-time code, not
            // from the passphrase, so a passphrase change leaves them alone.
            rotation: entries.rotation.clone(),
        },
        uploads,
        deks,
    })
}

/// What the sync cycle should do about a local vault record that the
/// workspace does not know about yet.
pub enum VaultMigration {
    /// Nothing to do: already migrated, a legacy server, no local record, or
    /// a record that cannot be uploaded yet.
    None,
    /// The workspace already has a vault this device's record did not
    /// create. Recorded as meta `vault_conflict`; the user resolves it by
    /// unlocking from the workspace entries (which clears the flag when the
    /// two turn out to be the same vault).
    Conflict,
    /// Seed the workspace vault with this body.
    Upload(SetupPayload),
}

/// Decides [`VaultMigration`] from the store alone — the pure half of the
/// sync cycle's migration hook.
///
/// A record without a DEK check is skipped **silently**: it predates the
/// check and will gain one on its next passphrase/recovery unlock, at which
/// point a later cycle picks it up.
pub fn vault_migration_plan(store: &Store) -> Result<VaultMigration, String> {
    let meta =
        |k: &str| crate::migrate::get_meta_i64_opt(&store.conn, k).map_err(|e| e.to_string());
    if meta("vault_migrated")?.is_some() || meta("vault_server_legacy")?.is_some() {
        return Ok(VaultMigration::None);
    }
    let Some(json) = store.vault_record().map_err(|e| e.to_string())? else {
        return Ok(VaultMigration::None);
    };
    let Ok(record) = VaultRecord::from_json(&json) else {
        return Ok(VaultMigration::None);
    };
    if crate::migrate::get_meta_i64(&store.conn, "vault_generation", 0) > 0 {
        return Ok(VaultMigration::Conflict);
    }
    Ok(match migration_payload(&record) {
        Ok(payload) => VaultMigration::Upload(payload),
        Err(_) => VaultMigration::None,
    })
}

/// Records the outcome of a migration attempt: `vault_migrated` once the
/// workspace holds this device's keys, `vault_conflict` when it already held
/// someone else's.
pub fn record_vault_migration(store: &Store, key: &str) -> Result<(), String> {
    crate::migrate::set_meta_i64(&store.conn, key, 1).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Vault invites
//
// How a workspace owner lets an invited member into the vault. The owner
// wraps the NEWEST DEK under a freshly generated one-time code and attaches
// that wrap to the invitation; the invitee opens it with the code and, in the
// same breath, replaces it with a wrap under a passphrase only they know.
//
// The code travels out of band (the owner reads it out) and is never stored:
// the server only ever sees wraps it cannot open, and the invite wrap is
// deleted the moment the member's own wrap takes its place.
// ---------------------------------------------------------------------------

/// The invite wrap: the body of `POST …/vault/invites/{id}` and — under the
/// server's `dekWrappedInvite` name — the response of the matching `GET`.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InviteWrap {
    pub generation: u32,
    pub kdf_params: KdfParams,
    /// Sent as `dekWrapped`; the alias also accepts the `dekWrappedInvite`
    /// the fetch endpoint answers with, so one struct serves both directions.
    #[serde(alias = "dekWrappedInvite")]
    pub dek_wrapped: String,
    pub dek_check: String,
}

/// Wraps `dek` under a fresh one-time invite code, returning the code
/// (dash-grouped, for the owner to pass on) and the wrap to attach.
pub fn make_invite_wrap(dek: &Dek, generation: u32) -> (String, InviteWrap) {
    let code = crate::vault::recovery::InviteCode::generate();
    let kdf_params = KdfParams::new_default();
    let kek = crate::vault::kdf::derive_kek(
        &crate::vault::recovery::InviteCode::normalize(code.as_str()),
        &kdf_params,
    )
    .expect("KdfParams::new_default() always produces valid Argon2 parameters");
    let wrap = InviteWrap {
        generation,
        kdf_params,
        dek_wrapped: STANDARD.encode(crate::vault::kdf::wrap_dek(&kek, dek)),
        dek_check: STANDARD.encode(crate::vault::make_dek_check(dek)),
    };
    (code.as_str().to_string(), wrap)
}

/// Opens an [`InviteWrap`] with the code the owner handed over. Accepts the
/// code however the user typed it (same normalization as a recovery key) and
/// proves the unwrapped DEK is the one the wrap was built from before handing
/// it back.
///
/// Every failure collapses to the same message on purpose: a rejected code
/// must not reveal *how* it was wrong.
pub fn open_invite_wrap(wrap: &InviteWrap, code: &str) -> Result<Dek, String> {
    let invalid = || "invalid invite code".to_string();
    // Server-supplied parameters, so bounded before they reach Argon2 — and
    // reported as the same generic failure as everything else here, since a
    // rejected wrap must not say WHY it was rejected.
    if !wrap.kdf_params.is_within_limits() {
        return Err(invalid());
    }
    let normalized = crate::vault::recovery::InviteCode::normalize(code);
    let kek =
        crate::vault::kdf::derive_kek(&normalized, &wrap.kdf_params).map_err(|_| invalid())?;
    let wrapped = STANDARD.decode(&wrap.dek_wrapped).map_err(|_| invalid())?;
    let dek = crate::vault::kdf::unwrap_dek(&kek, &wrapped).map_err(|_| invalid())?;
    let check = STANDARD.decode(&wrap.dek_check).map_err(|_| invalid())?;
    match crate::vault::aead::open(&dek, crate::vault::DEK_CHECK_AAD, &check) {
        Ok(pt) if pt == crate::vault::DEK_CHECK_MAGIC => Ok(dek),
        _ => Err(invalid()),
    }
}

/// The caller's own wrap for `generation` under `passphrase`: the body that
/// replaces the invite wrap on accept, and the entry cached locally
/// afterwards. Fresh KDF params, so it shares nothing with the invite wrap
/// beyond the DEK itself.
pub fn my_entry_for(dek: &Dek, generation: u32, passphrase: &str) -> MyEntryWire {
    let kdf_params = KdfParams::new_default();
    let kek = crate::vault::kdf::derive_kek(passphrase, &kdf_params)
        .expect("KdfParams::new_default() always produces valid Argon2 parameters");
    MyEntryWire {
        generation,
        kdf_params,
        dek_wrapped: STANDARD.encode(crate::vault::kdf::wrap_dek(&kek, dek)),
        dek_check: STANDARD.encode(crate::vault::make_dek_check(dek)),
    }
}

/// How the user identified an invitation.
///
/// Nobody ever sees an invitation's numeric id — the share page hands out a
/// link, `https://<server>/invite/<token>` — but the vault endpoints are keyed
/// by that id. So both dialogs take one free-form field and this decides what
/// was pasted; a `Token` is turned into an id by `vault_invite_resolve`.
#[derive(Debug, PartialEq, Eq)]
pub enum InvitationRef {
    Id(u64),
    Token(String),
}

/// Bare digits are an id; anything carrying an `/invite/<token>` segment is
/// that token (query and fragment stripped); anything else is taken as a bare
/// token, since that is what a user copying "just the code out of the link"
/// ends up with.
pub fn parse_invitation_ref(input: &str) -> Result<InvitationRef, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("invitation: nothing entered".to_string());
    }
    if let Ok(id) = trimmed.parse::<u64>() {
        return Ok(InvitationRef::Id(id));
    }
    let token = match trimmed.split("/invite/").nth(1) {
        Some(rest) => rest
            .split(['/', '?', '#'])
            .next()
            .unwrap_or_default()
            .trim(),
        None => trimmed,
    };
    if token.is_empty() {
        return Err("invitation: no token in that link".to_string());
    }
    // The token is interpolated straight into a request path, so restrict it
    // to what an invite token can actually be. Without this, a pasted `../`
    // (or a space, or a `?`) would reshape the URL the resolve call hits.
    if !token
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err("invitation: that does not look like an invitation link".to_string());
    }
    Ok(InvitationRef::Token(token.to_string()))
}

/// Merges a freshly accepted wrap into the cached entries, replacing any entry
/// that already covers the same generation (accepting a second invitation for
/// a generation this device already holds must not leave two wraps behind).
/// The recovery half is untouched — an invited member never gets one.
///
/// A pending ROTATION wrap for that same generation is dropped: the server
/// deletes it in the same transaction that stores the caller's own key
/// (`PUT …/vault/keys/me`), so keeping it would leave the UI asking for a
/// rotation code that is already spent.
pub fn merge_my_entry(entries: &VaultEntries, entry: MyEntryWire) -> Result<VaultEntries, String> {
    let generation = entry.generation;
    let parsed = MyEntry::try_from(entry)?;
    let mut mine: Vec<MyEntry> = entries
        .mine
        .iter()
        .filter(|e| e.generation != generation)
        .cloned()
        .collect();
    mine.push(parsed);
    mine.sort_by_key(|e| e.generation);
    Ok(VaultEntries {
        mine,
        recovery: entries.recovery.clone(),
        rotation: entries
            .rotation
            .iter()
            .filter(|e| e.generation != generation)
            .cloned()
            .collect(),
    })
}

/// Everything an accepted invitation settles locally: the merged entry cache,
/// and — for a device that has no vault record of its own yet — the mirrored
/// record that `vault_status`, biometric enrolment and `ensure_dek_check` all
/// read. Pure, so the command layer only has to write what it returns.
pub struct AcceptedInvite {
    pub entries: VaultEntries,
    pub record: Option<VaultRecord>,
    /// Whether this accept leaves the device CONFLICTED — see
    /// [`accept_invite_entry`]. `true` means "set `vault_conflict`, and do
    /// not claim `vault_migrated`"; `false` means the opposite.
    pub conflict: bool,
}

/// Folds `entry` into `cached` and, when `local` is `None`, mirrors the
/// generation-1 entry into a local record the same way an unlock from the
/// workspace entries does ([`apply_entry_unlock`]).
///
/// Accepting an invitation proves nothing about a vault this device set up on
/// its own: the workspace DEK it just installed and the local record's DEK can
/// be two different keys. So the accept is only conflict-FREE when there is
/// nothing to disagree with (no local record — the mirrored one IS the
/// workspace's) or when the local record provably holds the same DEK
/// ([`same_vault`], which falls back to opening the record with the new
/// passphrase for a record that predates `dek_check`).
///
/// Otherwise `conflict` is `true` and the flag is SET rather than merely left
/// alone: this device now holds two vaults' keys at once, and every sweep
/// that seals under "the newest generation" — [`reseal_lagging_notes`] above
/// all — has to know it before it touches a note. Its banner also keeps
/// telling the truth: notes protected on this device before joining remain
/// sealed under this device's own key.
pub fn accept_invite_entry(
    cached: Option<&VaultEntries>,
    local: Option<&VaultRecord>,
    entry: MyEntryWire,
    dek: &Dek,
    passphrase: &str,
) -> Result<AcceptedInvite, String> {
    let base = cached.cloned().unwrap_or_default();
    let entries = merge_my_entry(&base, entry)?;
    let (record, conflict) = match local {
        None => (mirrored_record(&entries), false),
        Some(rec) => (
            None,
            !same_vault(rec, dek, &VaultSecret::Passphrase(passphrase)),
        ),
    };
    Ok(AcceptedInvite {
        entries,
        record,
        conflict,
    })
}

/// Persists everything [`accept_invite_entry`] decided, in one store scope:
/// the merged cache, the mirrored record (if any), and the two meta flags.
///
/// A conflict-free accept claims `vault_migrated` (the workspace holds a key
/// for this caller, so the migration hook has nothing left to upload) and
/// clears `vault_conflict`. A CONFLICTED accept does neither: it sets
/// `vault_conflict` — so the re-seal sweep stands down — and deliberately
/// leaves `vault_migrated` unset, so the sync hook keeps re-marking the
/// conflict on every cycle instead of quietly forgetting it.
pub fn apply_accepted_invite(store: &Store, accepted: &AcceptedInvite) -> Result<(), String> {
    store
        .set_vault_entries(&accepted.entries.to_json())
        .map_err(|e| e.to_string())?;
    if let Some(record) = &accepted.record {
        store
            .set_vault_record(&record.to_json())
            .map_err(|e| e.to_string())?;
    }
    if accepted.conflict {
        crate::migrate::set_meta_i64(&store.conn, "vault_conflict", 1).map_err(|e| e.to_string())
    } else {
        crate::migrate::delete_meta(&store.conn, "vault_conflict").map_err(|e| e.to_string())?;
        crate::migrate::set_meta_i64(&store.conn, "vault_migrated", 1).map_err(|e| e.to_string())
    }
}

/// Persists one redeemed rotation generation: the caller's own wrap folded
/// into the cache, nothing else.
///
/// Deliberately does NOT touch `vault_conflict`. Redeeming a rotation code
/// proves the caller can open the WORKSPACE's newer key — it says nothing
/// about whether this device's own record is that same vault, which is
/// exactly what the flag records. A conflicted device that redeems stays
/// conflicted, and [`reseal_lagging_notes`] keeps standing down.
pub fn apply_rotation_redeem(store: &Store, entry: MyEntryWire) -> Result<(), String> {
    let cached = cached_vault_entries(store)?.unwrap_or_default();
    store
        .set_vault_entries(&merge_my_entry(&cached, entry)?.to_json())
        .map_err(|e| e.to_string())
}

/// How a conflicted device's locally sealed notes leave its own vault.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConflictMode {
    /// Re-seal under the workspace's newest generation.
    Merge,
    /// Store as plaintext (`protected = 0`) — EXCEPT for a note that sits
    /// inside a locked folder, which is merged instead. Folder locks are the
    /// user's standing instruction and are never changed here, and
    /// `protected = 0` under a locked ancestor is a state
    /// [`set_note_protected`] refuses outright: it would push plaintext to
    /// the server and be silently re-sealed on the next edit.
    Unprotect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictOutcome {
    pub changed: u32,
    /// Notes neither the local nor a workspace key opened — left untouched.
    pub skipped: u32,
}

/// What the resolve command reads under the store lock before opening anything.
pub struct ConflictInputs {
    pub entries: VaultEntries,
    pub record: VaultRecord,
}

pub fn load_conflict_inputs(store: &Store) -> Result<ConflictInputs, String> {
    if crate::migrate::get_meta_i64_opt(&store.conn, "vault_conflict")
        .map_err(|e| e.to_string())?
        .is_none()
    {
        return Err("vault: no conflict to resolve".to_string());
    }
    let entries =
        cached_vault_entries(store)?.ok_or_else(|| "vault: no workspace keys".to_string())?;
    let record = load_vault_record(store)?;
    Ok(ConflictInputs { entries, record })
}

/// Opens both sides without touching the store: the workspace ring with its
/// passphrase (every generation it opens) and the device's own record with
/// its own secret. Argon2 runs here, so callers hold no lock.
pub fn open_conflict_sides(
    inputs: &ConflictInputs,
    workspace_passphrase: &str,
    local: &VaultSecret<'_>,
) -> Result<(Vec<(u32, Dek)>, Dek), String> {
    let ring = unlock_entries_with_passphrase(&inputs.entries, workspace_passphrase)?;
    let local_dek = local
        .open(&inputs.record)
        .map_err(|_| "vault: local record does not open".to_string())?;
    Ok((ring, local_dek))
}

/// Moves every note sealed under `local_dek` out of the device's own vault:
/// re-sealed under the ring's newest generation (`Merge`) or written back as
/// plaintext (`Unprotect`). Notes a workspace generation already opens stay
/// as they are; notes nothing opens are counted and left alone. One
/// transaction per note.
///
/// Folder locks are never changed here — which is why `Unprotect` is not
/// unconditional: a note inside a locked folder is MERGED instead (sealed,
/// stamped, dirty) and counted in `changed` all the same. Unprotecting it
/// would leave `protected = 0` below a locked ancestor, exactly the state
/// [`set_note_protected`]'s "note is protected by its folder" refusal
/// exists to prevent.
///
/// Trashed protected notes are in the work list too (see
/// [`Store::protected_note_ids`]): the device's own record is replaced right
/// after this, so anything left sealed under the local DEK could never be
/// opened again.
///
/// Refuses outright, before any note moves, when the ring's newest
/// generation is behind the workspace's ([`guard_seal_generation`]).
pub fn resolve_conflict(
    store: &Store,
    ring: &[(u32, Dek)],
    local_dek: &Dek,
    mode: ConflictMode,
) -> Result<ConflictOutcome, String> {
    let (newest, newest_dek) = ring
        .iter()
        .max_by_key(|(g, _)| *g)
        .map(|(g, d)| (*g, d))
        .ok_or_else(|| "vault: no workspace keys".to_string())?;
    // `Merge` seals under `newest`, and so does `Unprotect` for a note under
    // a locked folder — both go through the same choke point every other seal
    // does. Without it a device whose cached entries stop at generation N-1
    // while the workspace is already on N would move its notes onto the
    // RETIRED key. The conflict exemption inside the guard does not fire
    // here: this ring came from the verified workspace entries, so
    // `ring_key_is_the_workspaces` says yes and the normal comparison applies.
    guard_seal_generation(store, newest_dek, newest)?;
    let mut outcome = ConflictOutcome::default();
    if ring.iter().any(|(_, d)| d.expose() == local_dek.expose()) {
        return Ok(outcome); // one vault on both sides — nothing to move
    }
    for id in store.protected_note_ids().map_err(|e| e.to_string())? {
        let Some(stored) = store.load_note_content(&id).map_err(|e| e.to_string())? else {
            continue;
        };
        // The LOCAL key first: on a conflicted device most protected notes
        // are the ones this resolution exists to move, so asking the ring
        // first would run the whole ring's AEAD over every one of them before
        // the key that actually opens it. Same three outcomes as asking the
        // other way round — no two distinct keys open the same ciphertext,
        // and a local key that IS in the ring already returned above.
        let plaintext = match open_content(local_dek, &id, &stored) {
            Ok(plaintext) => plaintext,
            Err(_) => {
                // Not this device's. Either a workspace generation opens it
                // (already the workspace's — leave it) or nothing does.
                if !ring
                    .iter()
                    .any(|(_, d)| open_content(d, &id, &stored).is_ok())
                {
                    outcome.skipped += 1;
                }
                continue;
            }
        };
        // A locked ancestor overrides `Unprotect` — see the note on
        // [`ConflictMode::Unprotect`]. Asked per note rather than hoisted:
        // the work list spans the whole tree.
        let mode = match mode {
            ConflictMode::Unprotect if has_locked_ancestor_folder(store, &id)? => {
                ConflictMode::Merge
            }
            m => m,
        };
        let tx = store
            .conn
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;
        match mode {
            ConflictMode::Merge => {
                store
                    .set_content_silent(&id, &seal_content(newest_dek, &id, &plaintext))
                    .map_err(|e| e.to_string())?;
                store
                    .set_note_key_gen(&id, Some(newest))
                    .map_err(|e| e.to_string())?;
            }
            ConflictMode::Unprotect => {
                store
                    .set_content_silent(&id, &plaintext)
                    .map_err(|e| e.to_string())?;
                store
                    .set_note_protected(&id, false)
                    .map_err(|e| e.to_string())?;
                store
                    .set_note_key_gen(&id, None)
                    .map_err(|e| e.to_string())?;
            }
        }
        store
            .mark_note_dirty_if_syncing(&id)
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        outcome.changed += 1;
    }
    Ok(outcome)
}

/// After the notes moved: the device's record becomes the mirrored workspace
/// record (or an empty placeholder when the cache holds no generation-1
/// entry — the cache alone keeps `vault_exists` true), the conflict flag
/// goes, and the device counts as migrated.
pub fn finish_conflict_resolution(store: &Store, entries: &VaultEntries) -> Result<(), String> {
    let json = mirrored_record(entries)
        .map(|r| r.to_json())
        .unwrap_or_default();
    store.set_vault_record(&json).map_err(|e| e.to_string())?;
    crate::migrate::delete_meta(&store.conn, "vault_conflict").map_err(|e| e.to_string())?;
    crate::migrate::set_meta_i64(&store.conn, "vault_migrated", 1).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Key rotation
//
// When a member is removed the workspace's key has to change: the removed
// member still knows the old DEK, and every note that stays sealed under it
// stays readable to them. The rotating client mints a NEW DEK and hands the
// server one wrap per remaining member — its own under its own passphrase
// (`kind: "own"`), everyone else's under a fresh one-time ROTATION code
// (`kind: "code"`, the invite wrap reused), which the owner passes on out of
// band. Members redeem their code with their passphrase
// (`vault_rotation_redeem`), and existing ciphertext moves to the new
// generation lazily ([`reseal_lagging_notes`]).
//
// The workspace's single recovery wrap can only be produced by whoever holds
// the recovery key: the creator adds it inline when they rotate, and after
// someone else rotated they add it afterwards ([`recovery_followup`]).
// ---------------------------------------------------------------------------

/// One `keys[]` element of `POST …/vault/rotate`: a wrap of the NEW DEK for
/// one member. `kind` is `"own"` (a passphrase wrap, stored as that member's
/// key row) or `"code"` (a rotation-code wrap, parked until the member
/// redeems it).
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RotateKeyWire {
    pub user_id: u64,
    pub kind: String,
    pub kdf_params: KdfParams,
    pub dek_wrapped: String,
    pub dek_check: String,
}

/// The body of `POST …/vault/rotate`. `recovery` is `null` when the rotating
/// user does not hold the recovery key — the creator supplies it later
/// through `POST …/vault/recovery`.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RotatePayload {
    pub generation: u32,
    pub keys: Vec<RotateKeyWire>,
    pub recovery: Option<RecoveryPayload>,
}

impl RotatePayload {
    /// The rotating caller's own entry, as the cache stores it. Read back off
    /// the payload rather than derived a second time — the Argon2 derivation
    /// behind it is deliberately expensive.
    pub fn own_entry(&self) -> Option<MyEntryWire> {
        self.keys
            .iter()
            .find(|k| k.kind == "own")
            .map(|k| MyEntryWire {
                generation: self.generation,
                kdf_params: k.kdf_params.clone(),
                dek_wrapped: k.dek_wrapped.clone(),
                dek_check: k.dek_check.clone(),
            })
    }
}

/// Who a rotation has to wrap the new key for, and which generation it
/// becomes — the whole decision the `vault_rotate` command used to make
/// inline, between two network calls.
pub struct RotationPlan {
    /// The generation the new key takes.
    pub new_generation: u32,
    /// Every OTHER member, deduplicated and ascending. The caller is excluded
    /// here rather than inside [`rotation_payload`]: the members listing
    /// includes them, and wrapping them twice makes the server reject the
    /// whole rotation.
    pub others: Vec<u64>,
}

/// Decides [`RotationPlan`] from the workspace's current generation and its
/// member list.
///
/// `current` is what the last pull cached (meta `vault_generation`); the new
/// generation is the next one up, saturating rather than wrapping — a `u32`
/// that reached its maximum would otherwise roll back to a generation whose
/// key some ex-member still knows.
pub fn rotation_plan(current: u32, members: &[u64], me: u64) -> RotationPlan {
    let mut others: Vec<u64> = members.iter().copied().filter(|id| *id != me).collect();
    others.sort_unstable();
    others.dedup();
    RotationPlan {
        new_generation: current.saturating_add(1),
        others,
    }
}

/// Builds the rotation body for `new_dek`: the caller's own wrap under their
/// passphrase, one rotation-code wrap per other member, and — when the caller
/// holds the recovery key — the workspace recovery wrap for the new
/// generation. Returns the body plus one `(user id, code)` per other member;
/// those codes exist nowhere else and are never stored.
pub fn rotation_payload(
    new_generation: u32,
    new_dek: &Dek,
    own: (u64, &str),
    others: &[u64],
    recovery: Option<&str>,
) -> Result<(RotatePayload, Vec<(u64, String)>), String> {
    let (own_id, passphrase) = own;
    let own_entry = my_entry_for(new_dek, new_generation, passphrase);
    let mut keys = vec![RotateKeyWire {
        user_id: own_id,
        kind: "own".to_string(),
        kdf_params: own_entry.kdf_params,
        dek_wrapped: own_entry.dek_wrapped,
        dek_check: own_entry.dek_check,
    }];
    let mut codes = Vec::new();
    for &user_id in others {
        // The members listing includes the caller; wrapping them twice would
        // make the server reject the whole rotation.
        if user_id == own_id {
            continue;
        }
        let (code, wrap) = make_invite_wrap(new_dek, new_generation);
        keys.push(RotateKeyWire {
            user_id,
            kind: "code".to_string(),
            kdf_params: wrap.kdf_params,
            dek_wrapped: wrap.dek_wrapped,
            dek_check: wrap.dek_check,
        });
        codes.push((user_id, code));
    }
    let recovery = match recovery {
        None => None,
        Some(key) => Some(recovery_payload_for(new_dek, key)?),
    };
    Ok((
        RotatePayload {
            generation: new_generation,
            keys,
            recovery,
        },
        codes,
    ))
}

/// One generation's recovery wrap, built the same way [`crate::vault::setup`]
/// builds a vault's: fresh salt, recovery KDF params, normalized key.
fn recovery_payload_for(dek: &Dek, recovery_key: &str) -> Result<RecoveryPayload, String> {
    let (salt, wrapped, check) =
        crate::vault::wrap_under_recovery(dek, recovery_key).map_err(String::from)?;
    Ok(RecoveryPayload {
        recovery_salt: STANDARD.encode(salt),
        dek_wrapped_recovery: STANDARD.encode(&wrapped),
        dek_check: STANDARD.encode(&check),
    })
}

/// One recovery wrap per ring generation under a freshly generated recovery
/// key — the body of "create my own recovery key". Nothing is verified
/// against the cache: this owner holds no recovery entries yet.
pub fn recovery_create_payloads(
    deks: &[(u32, Dek)],
    recovery_key: &str,
) -> Result<Vec<(u32, RecoveryPayload)>, String> {
    deks.iter()
        .map(|(g, dek)| recovery_payload_for(dek, recovery_key).map(|p| (*g, p)))
        .collect()
}

/// Generations the workspace wrapped for this caller under a one-time
/// rotation code and that they hold no own wrap for yet — ascending. Backs
/// `vault_status.rotation_code` and drives [`rotation_redeem_entries`].
pub fn pending_rotation_generations(entries: &VaultEntries) -> Vec<u32> {
    let mut gens: Vec<u32> = entries
        .rotation
        .iter()
        .map(|r| r.generation)
        .filter(|g| !entries.mine.iter().any(|m| m.generation == *g))
        .collect();
    gens.sort_unstable();
    gens.dedup();
    gens
}

/// Opens the pending rotation wraps `code` fits and re-wraps each DEK under
/// `passphrase` — one `(generation, DEK, own wrap)` per generation it opened,
/// ascending, ready to be PUT and then cached.
///
/// Deliberately PER GENERATION, not all-or-nothing: two rotations while a
/// member was away leave that member two wraps behind two DIFFERENT one-time
/// codes. Failing the whole redemption on the first wrap a code does not open
/// would make both codes useless and strand them on `rotation_code` forever.
/// So each code redeems its own generation and leaves the rest pending for
/// the next one.
///
/// `Err("invalid rotation code")` only when NOTHING opened — and every
/// failure collapses into that one message: a rejected code must not reveal
/// *how* it was wrong.
pub fn rotation_redeem_entries(
    entries: &VaultEntries,
    code: &str,
    passphrase: &str,
) -> Result<Vec<(u32, Dek, MyEntryWire)>, String> {
    let pending = pending_rotation_generations(entries);
    if pending.is_empty() {
        return Err("no rotation pending".to_string());
    }
    let mut out = Vec::with_capacity(pending.len());
    for generation in pending {
        let Some(entry) = entries.rotation.iter().find(|r| r.generation == generation) else {
            continue;
        };
        let wire = MyEntryWire::from(entry);
        let wrap = InviteWrap {
            generation,
            kdf_params: wire.kdf_params,
            dek_wrapped: wire.dek_wrapped,
            dek_check: wire.dek_check,
        };
        let Ok(dek) = open_invite_wrap(&wrap, code) else {
            continue; // a different rotation's code — leave it pending
        };
        let own = my_entry_for(&dek, generation, passphrase);
        out.push((generation, dek, own));
    }
    if out.is_empty() {
        return Err("invalid rotation code".to_string());
    }
    Ok(out)
}

/// Generations this caller holds an own wrap for that the workspace has no
/// recovery wrap for. Non-empty only after somebody rotated without the
/// recovery key — the creator's follow-up work list, and the source of
/// `vault_status.recovery_missing`.
pub fn generations_missing_recovery(entries: &VaultEntries) -> Vec<u32> {
    let mut gens: Vec<u32> = entries
        .mine
        .iter()
        .map(|m| m.generation)
        .filter(|g| !entries.recovery.iter().any(|r| r.generation == *g))
        .collect();
    gens.sort_unstable();
    gens.dedup();
    gens
}

/// Proves `passphrase` is the one the workspace currently wraps this caller's
/// key under, by requiring it to open the NEWEST cached `mine` entry — the
/// wrap every rotation and redemption builds on. An empty cache has nothing
/// to prove anything against and is rejected.
pub fn verify_newest_passphrase(entries: &VaultEntries, passphrase: &str) -> Result<(), String> {
    let newest = entries
        .mine
        .iter()
        .map(|e| e.generation)
        .max()
        .ok_or_else(|| "wrong passphrase".to_string())?;
    let opened = unlock_entries_with_passphrase(entries, passphrase)?;
    match opened.iter().any(|(g, _)| *g == newest) {
        true => Ok(()),
        false => Err("wrong passphrase".to_string()),
    }
}

/// Proves `recovery_key` is this vault's recovery key by opening the cached
/// generation-1 recovery wrap with it.
pub fn verify_recovery_key(entries: &VaultEntries, recovery_key: &str) -> Result<(), String> {
    let opened = unlock_entries_with_recovery(entries, recovery_key)?;
    match opened.iter().any(|(g, _)| *g == 1) {
        true => Ok(()),
        false => Err("wrong recovery key".to_string()),
    }
}

/// The creator's recovery follow-up: after verifying `recovery_key`, one
/// `POST …/vault/recovery` body per generation that lacks a recovery wrap and
/// whose DEK the ring can actually hand over. A generation nobody unlocked
/// cannot be wrapped and is simply skipped — the next unlock picks it up.
pub fn recovery_followup(
    entries: &VaultEntries,
    deks: &[(u32, Dek)],
    recovery_key: &str,
) -> Result<Vec<(u32, RecoveryPayload)>, String> {
    verify_recovery_key(entries, recovery_key)?;
    let mut out = Vec::new();
    for generation in generations_missing_recovery(entries) {
        let Some((_, dek)) = deks.iter().find(|(g, _)| *g == generation) else {
            continue;
        };
        out.push((generation, recovery_payload_for(dek, recovery_key)?));
    }
    Ok(out)
}

/// Folds a freshly uploaded recovery wrap into the cache, replacing any entry
/// for the same generation.
pub fn merge_recovery_entry(
    entries: &VaultEntries,
    generation: u32,
    payload: &RecoveryPayload,
) -> Result<VaultEntries, String> {
    let parsed = RecoveryEntry::try_from(RecoveryEntryWire {
        generation,
        recovery_salt: payload.recovery_salt.clone(),
        dek_wrapped_recovery: payload.dek_wrapped_recovery.clone(),
        dek_check: Some(payload.dek_check.clone()),
    })?;
    let mut recovery: Vec<RecoveryEntry> = entries
        .recovery
        .iter()
        .filter(|e| e.generation != generation)
        .cloned()
        .collect();
    recovery.push(parsed);
    recovery.sort_by_key(|e| e.generation);
    Ok(VaultEntries {
        mine: entries.mine.clone(),
        recovery,
        rotation: entries.rotation.clone(),
    })
}

/// Moves up to `batch` protected notes that are still sealed under an older
/// key generation onto the ring's newest one, marking each dirty so the next
/// sync cycle pushes the re-sealed ciphertext.
///
/// A row whose `key_gen` is NULL is on the work list because schema v15 added
/// the column without backfilling it, and the global rule reads NULL as
/// generation 1 ([`VaultState::dek_for`]). Such a row is only STAMPED with
/// the newest generation when that already IS its generation — no re-seal, no
/// dirty flag, no `updated_at` change. Re-sealing it would burn a new nonce
/// on the same key, re-upload the note and move its "last edited" date for no
/// cryptographic gain. It still counts as handled, so the work list drains;
/// on a ring whose newest generation is 1 the sweep therefore never touches
/// content at all.
///
/// Otherwise deliberately best-effort per note: a generation this ring cannot
/// open (a member who joined at a later generation never sees the older DEKs)
/// and a row that will not open are skipped rather than failing the batch —
/// the note keeps its old generation and simply stays on the work list.
///
/// **Refuses to run at all while meta `vault_conflict` is set.** On a
/// conflicted device the ring can hold BOTH this device's own generation-1
/// DEK and the workspace's newer generations, and "newest" is then the
/// workspace's — re-sealing would silently move this device's private notes
/// under the workspace key, handing them to every workspace member. Nothing
/// is re-sealed until an unlock proves the two are one vault (which clears
/// the flag) or the (deferred) merge lands.
pub fn reseal_lagging_notes(
    store: &Store,
    vault: &VaultState,
    batch: usize,
) -> Result<usize, String> {
    if crate::migrate::get_meta_i64_opt(&store.conn, "vault_conflict")
        .map_err(|e| e.to_string())?
        .is_some()
    {
        return Ok(0);
    }
    let (Some(newest_dek), Some(newest)) = (vault.dek(), vault.newest_generation()) else {
        return Ok(0);
    };
    let ids = store
        .notes_with_key_gen_below(newest, batch)
        .map_err(|e| e.to_string())?;
    let mut done = 0;
    for id in ids {
        let old_gen = store.note_key_gen(&id).map_err(|e| e.to_string())?;
        if old_gen.unwrap_or(1) == newest {
            // Already sealed under the newest key — only the column was blank.
            store
                .set_note_key_gen(&id, Some(newest))
                .map_err(|e| e.to_string())?;
            done += 1;
            continue;
        }
        let Some(stored) = store.load_note_content(&id).map_err(|e| e.to_string())? else {
            continue;
        };
        let Some(old_dek) = vault.dek_for(old_gen) else {
            continue; // not ours to re-seal
        };
        let Ok(plaintext) = open_content(old_dek, &id, &stored) else {
            continue;
        };
        // One transaction per note: the new ciphertext, its generation stamp
        // and the dirty flag land together or not at all. A note left with
        // generation N's ciphertext and a stamp of N+1 would be unopenable
        // for good.
        let tx = store
            .conn
            .unchecked_transaction()
            .map_err(|e| e.to_string())?;
        store
            .set_content_silent(&id, &seal_content(newest_dek, &id, &plaintext))
            .map_err(|e| e.to_string())?;
        store
            .set_note_key_gen(&id, Some(newest))
            .map_err(|e| e.to_string())?;
        store
            .mark_note_dirty_if_syncing(&id)
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        done += 1;
    }
    Ok(done)
}

/// Whether this user holds a recovery key for this context's vault at all —
/// the gate for every recovery-key control in the UI.
///
/// A local vault's recovery key was minted on this device, so its owner always
/// holds one. On a workspace, three things can prove it:
///
/// - the workspace handed back a recovery wrap for this caller, or
/// - this device's own record carries recovery material. That covers the
///   creator between `vault_setup` and the first pull that caches the
///   entries, and any window where the cache is missing or unparsable — they
///   were shown a recovery key and it still works. An invitee's MIRRORED
///   record has a zero salt and an empty recovery wrap, so it does not count.
///
/// Otherwise the recovery paths are a dead end and must not be offered.
pub fn vault_recovery_holder(
    entries: Option<&VaultEntries>,
    record: Option<&VaultRecord>,
    is_server_context: bool,
) -> bool {
    !is_server_context
        || entries.is_some_and(|e| !e.recovery.is_empty())
        || record.is_some_and(|r| !r.dek_wrapped_recovery.is_empty())
}

/// Whether the Security page offers "create a recovery key": a server
/// context, an owner, no recovery set of their own yet, an unlocked ring
/// (the wraps are made from the live DEKs), and no local/workspace conflict.
///
/// `conflict` (meta `vault_conflict`, surfaced as `VaultStatusFlags.conflict`
/// / `VaultStatus.conflict`) rules out a device holding two vaults' keys at
/// once: which key the live ring would wrap next is not provable there, and
/// uploading a recovery wrap of what might be this device's LOCAL key under
/// a workspace generation would hand out a "recovery" key that opens the
/// wrong vault.
pub fn recovery_eligible(
    is_server_context: bool,
    role: &str,
    recovery_holder: bool,
    unlocked: bool,
    conflict: bool,
) -> bool {
    is_server_context && role == "owner" && !recovery_holder && unlocked && !conflict
}

/// The store-derived halves of `vault_status`, read under one lock: whether a
/// vault exists for this context, whether the workspace migration hit a
/// conflict, and whether the recovery paths apply to this user.
pub struct VaultStatusFlags {
    pub exists: bool,
    pub conflict: bool,
    pub recovery_holder: bool,
    /// A rotation code is waiting to be redeemed — the workspace wrapped a
    /// newer generation for this caller under a one-time code.
    pub rotation_code: bool,
    /// This user holds the recovery key, and some generation of the vault has
    /// no recovery wrap yet (somebody else rotated). Only they can add it.
    pub recovery_missing: bool,
    /// The workspace's key generation as of the last pull (meta
    /// `vault_generation`, 0 for a local context). Handed out raw so the
    /// caller can pair it with the live ring — which lives behind a different
    /// lock — via [`seal_outdated`].
    pub server_generation: i64,
    /// Whether the key the ring would seal with is the WORKSPACE's rather
    /// than this device's own — [`ring_key_is_the_workspaces`] for the ring's
    /// newest generation. `false` for a locked vault. Only meaningful
    /// alongside `conflict`; see [`seal_outdated`].
    pub ring_is_workspace: bool,
    /// This user's role in the workspace as of the last pull (meta
    /// `workspace_role`); empty for a local context or before the first pull.
    /// Feeds [`recovery_eligible`], which only an "owner" ever satisfies.
    pub role: String,
}

/// `ring` is the ring's newest `(generation, DEK)`, or `None` while the vault
/// is locked. Passed in rather than read here because `VaultState` lives
/// behind a different mutex — the caller takes and drops that guard before
/// taking the store's.
pub fn vault_status_flags(
    store: &Store,
    is_server_context: bool,
    ring: Option<(u32, &Dek)>,
) -> Result<VaultStatusFlags, String> {
    let raw_record = store.vault_record().map_err(|e| e.to_string())?;
    // A record that will not parse is treated as absent for the recovery
    // question — exactly how `cached_vault_entries` treats an unparsable
    // cache. `exists` deliberately still counts it: something IS set up here,
    // and offering "set up a vault" over it would mint a second DEK.
    let record = raw_record
        .as_deref()
        .and_then(|json| VaultRecord::from_json(json).ok());
    let entries = cached_vault_entries(store)?;
    Ok(VaultStatusFlags {
        exists: vault_exists(store)?,
        conflict: crate::migrate::get_meta_i64_opt(&store.conn, "vault_conflict")
            .map_err(|e| e.to_string())?
            .is_some(),
        recovery_holder: vault_recovery_holder(
            entries.as_ref(),
            record.as_ref(),
            is_server_context,
        ),
        rotation_code: entries
            .as_ref()
            .is_some_and(|e| !pending_rotation_generations(e).is_empty()),
        server_generation: crate::migrate::get_meta_i64(&store.conn, "vault_generation", 0),
        ring_is_workspace: ring.is_some_and(|(generation, dek)| {
            ring_key_is_the_workspaces(entries.as_ref(), generation, dek)
        }),
        // Only the creator holds recovery wraps at all, so only they are ever
        // asked to fill a gap in them.
        recovery_missing: is_server_context
            && entries.as_ref().is_some_and(|e| {
                !e.recovery.is_empty() && !generations_missing_recovery(e).is_empty()
            }),
        role: crate::migrate::get_meta(&store.conn, "workspace_role")
            .map_err(|e| e.to_string())?
            .unwrap_or_default(),
    })
}

/// What `contexts_list` reads out of one context's own database: whether it
/// has a vault, and where that vault's workspace key ring stands. All three
/// fields come from the same short-lived `Store` handle, so a context's row
/// costs one open, not three.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextVaultInfo {
    pub exists: bool,
    pub generation: u32,
    pub rotation_pending: bool,
    /// The user's role in the workspace as of the last pull; "" for local
    /// contexts.
    pub role: String,
    /// Open invitations whose vault code was lost in a rotation (owners
    /// only).
    pub invites_needing_code: u32,
}

/// One context's vault state, read without switching into it — used by
/// `contexts_list` to decorate the Kontexte page. Opens and migrates a
/// throwaway `Store` handle on `path`; any failure (missing file, unreadable
/// DB, ...) reports the default "no vault, no generations" rather than
/// surfacing, since this is best-effort UI decoration, not a security gate —
/// the real gate is still `load_vault_record` on the context actually made
/// active. A local context has no key-ring meta rows and so reports
/// generation 0.
pub fn context_vault_info(path: &Path) -> ContextVaultInfo {
    (|| -> Result<ContextVaultInfo, String> {
        let store = Store::open(path).map_err(|e| e.to_string())?;
        crate::migrate::run_migrations(&store.conn).map_err(|e| e.to_string())?;
        let generation = u32::try_from(crate::migrate::get_meta_i64(
            &store.conn,
            "vault_generation",
            0,
        ))
        .unwrap_or(0);
        Ok(ContextVaultInfo {
            // Same rule as `vault_status.exists` ([`vault_exists`]): a member
            // who joined a workspace at generation >= 2 has no local record —
            // nothing is mirrored for them — but the cached wrap proves the
            // vault is there. Reading only the record would leave their row
            // saying "no vault" and hide every vault action on it.
            exists: vault_exists(&store)?,
            generation,
            rotation_pending: crate::migrate::get_meta_i64(
                &store.conn,
                "vault_rotation_pending",
                0,
            ) != 0,
            role: crate::migrate::get_meta(&store.conn, "workspace_role")
                .map_err(|e| e.to_string())?
                .unwrap_or_default(),
            invites_needing_code: crate::migrate::get_meta(&store.conn, "vault_invites")
                .map_err(|e| e.to_string())?
                .map(|json| invites_needing_code(&json, generation).len() as u32)
                .unwrap_or(0),
        })
    })()
    .unwrap_or_default()
}

/// `context_vault_change_passphrase` for a NON-active context: open that
/// context's own DB, verify `current` and re-wrap the DEK under `next` there
/// (the same rewrap [`vault_change_passphrase`] performs on the active
/// store), and discard the returned DEK. That context is not being unlocked
/// or switched into — `VaultState` (which always tracks the ACTIVE
/// context) must never be touched here.
///
/// The active context is refused: rewrapping it out from under the live
/// `VaultState`/`Store` split here would desync the two. The command layer
/// special-cases that id and reuses the existing `vault_change_passphrase`
/// command path instead (managed store + `vault.unlock(1, dek)`).
pub fn change_context_vault_passphrase(
    reg: &Registry,
    context_id: &str,
    current: &str,
    next: &str,
) -> Result<(), String> {
    if context_id == reg.active_id {
        return Err("active context: use vault_change_passphrase".to_string());
    }
    let entry = reg
        .contexts
        .iter()
        .find(|c| c.id == context_id)
        .ok_or_else(|| "unknown context".to_string())?;
    let store = Store::open(Path::new(&entry.path)).map_err(|e| e.to_string())?;
    crate::migrate::run_migrations(&store.conn).map_err(|e| e.to_string())?;
    vault_change_passphrase(&store, current, next)?;
    Ok(())
}

/// `note_set_protected`: encrypts or decrypts one note's stored content in
/// place, keeping `notes.protected` in sync with the physical content state.
///
/// `protected = true` seals under the ring's NEWEST generation (same as every
/// other sealing path). It refuses with `Err("vault locked")` when the ring is
/// empty and with `Err("vault: key generation outdated …")` when the
/// workspace has rotated past that generation (see [`guard_seal_generation`])
/// — but only when there is something to seal: a note that is ALREADY
/// protected is a no-op, checked before either guard, so re-asserting the
/// flag on a locked vault succeeds rather than erroring.
///
/// `protected = false` opens under the DEK the note was ACTUALLY sealed with
/// — see [`decrypt_note_in_place`] for its `"vault locked"` / `"key
/// generation not available"` split — and is also refused while the note is
/// inside a `locked` folder: the folder is the source of truth for that
/// note's protection until the folder itself is unlocked.
///
/// Transitioning to `protected = true` discards the note's existing revision
/// history (see [`encrypt_note_in_place`]) — v1 behavior, since
/// `note_revisions` is unencrypted.
pub fn set_note_protected(
    store: &Store,
    vault: &VaultState,
    id: &str,
    protected: bool,
) -> Result<(), String> {
    if protected {
        if !store.note_protected(id).map_err(|e| e.to_string())? {
            let (dek, generation) = vault
                .dek()
                .zip(vault.newest_generation())
                .ok_or_else(|| "vault locked".to_string())?;
            guard_seal_generation(store, dek, generation)?;
            // Seal + flip `protected` + mark dirty + purge the plaintext
            // revision history (v1: keeping it would defeat
            // encryption-at-rest, since note_revisions is unencrypted).
            encrypt_note_in_place(store, id, dek, generation)?;
        }
    } else {
        if has_locked_ancestor_folder(store, id)? {
            return Err("note is protected by its folder".to_string());
        }
        if store.note_protected(id).map_err(|e| e.to_string())? {
            decrypt_note_in_place(store, vault, id)?;
        }
    }
    Ok(())
}

/// `folder_set_locked`: locks or unlocks a folder, encrypting/decrypting the
/// notes in its subtree to match.
///
/// `locked = true` seals every currently-plaintext subtree note under the
/// ring's NEWEST generation, refusing up front with `Err("vault locked")` if
/// the ring is empty and with `Err("vault: key generation outdated …")` if the
/// workspace has rotated past that generation (matching every other sealing
/// path — see [`guard_seal_generation`]).
///
/// `locked = false` opens each note under the DEK it was ACTUALLY sealed
/// with. Which notes need decrypting — and whether each one really opens — is
/// determined and validated BEFORE `id` itself is flipped to unlocked in the
/// database: `Err("vault locked")` (ring empty) or `Err("key generation not
/// available")` (the ring lacks a note's generation, or the DEK it holds for
/// that generation belongs to another vault) leaves `id.locked` and every
/// note exactly as they were, rather than committing the folder open while a
/// note fails to decrypt.
///
/// v1 limitation: `notes.protected` tracks only physical ciphertext state,
/// not a separate "individually locked" intent, so unlocking a folder
/// decrypts every subtree note that has no *other* locked ancestor —
/// including a note that was individually protected while it happened to
/// live inside this now-unlocking folder. Acceptable for v1.
///
/// Locking (not unlocking) also discards each newly-encrypted note's
/// existing revision history, same rationale as [`set_note_protected`].
pub fn set_folder_locked(
    store: &Store,
    vault: &VaultState,
    id: &str,
    locked: bool,
) -> Result<(), String> {
    let note_ids = store.note_ids_in_subtree(id).map_err(|e| e.to_string())?;

    if locked {
        let (dek, generation) = vault
            .dek()
            .zip(vault.newest_generation())
            .ok_or_else(|| "vault locked".to_string())?;
        guard_seal_generation(store, dek, generation)?;
        store
            .set_folder_locked(id, true)
            .map_err(|e| e.to_string())?;
        for note_id in &note_ids {
            if !store.note_protected(note_id).map_err(|e| e.to_string())? {
                // Same transition as set_note_protected(id, true): seal +
                // flip `protected` + mark dirty + discard this note's now
                // encryption-defeating plaintext revision history.
                encrypt_note_in_place(store, note_id, dek, generation)?;
            }
        }
    } else {
        // Which notes will actually need decrypting once `id` itself is
        // unlocked? `..._except(id)` answers that WITHOUT `id`'s own `locked`
        // flag having been flipped yet — a note stays sealed only if some
        // OTHER ancestor is still locked.
        let mut to_decrypt: Vec<&String> = Vec::new();
        for note_id in &note_ids {
            if store.note_protected(note_id).map_err(|e| e.to_string())?
                && !has_locked_ancestor_folder_except(store, note_id, id)?
            {
                to_decrypt.push(note_id);
            }
        }
        // Validate every one of them ACTUALLY OPENS — BEFORE touching a
        // single row — so a note the ring cannot decrypt refuses the whole
        // operation up front instead of surfacing partway through the
        // per-note loop below, with `id.locked` already false.
        //
        // A generation lookup alone is not enough: on a device that holds two
        // vaults' keys at once (meta `vault_conflict`) the ring can carry a
        // generation number whose DEK belongs to the OTHER vault, so
        // `dek_for` answers `Some` and the AEAD open still fails. Trial-open
        // each note instead — it is a cheap symmetric operation, unlike the
        // Argon2 derivations the unlock paths avoid holding locks across.
        if !to_decrypt.is_empty() && !vault.is_unlocked() {
            return Err("vault locked".to_string());
        }
        for note_id in &to_decrypt {
            let gen = store.note_key_gen(note_id).map_err(|e| e.to_string())?;
            let unavailable = || "key generation not available".to_string();
            let dek = vault.dek_for(gen).ok_or_else(unavailable)?;
            let stored = store
                .load_note_content(note_id)
                .map_err(|e| e.to_string())?
                .unwrap_or_default();
            open_content(dek, note_id, &stored).map_err(|_| unavailable())?;
        }

        store
            .set_folder_locked(id, false)
            .map_err(|e| e.to_string())?;
        for note_id in to_decrypt {
            decrypt_note_in_place(store, vault, note_id)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Context registry
// ---------------------------------------------------------------------------

/// Registry snapshot for the frontend context switcher.
///
/// `vault_exists`/`biometric` are injected rather than computed here: each
/// check needs to open the *other* context's DB (or query the keychain),
/// which this pure/testable core has no business doing. `contexts_list`
/// (`commands.rs`) supplies real closures; every other caller — mutating a
/// context and handing back the fresh snapshot — keeps the cheap
/// [`to_infos`] wrapper, since the frontend re-fetches the vault flags via
/// its own `contexts.list()` call anyway.
pub fn to_infos_with(
    reg: &Registry,
    vault: impl Fn(&ContextEntry) -> ContextVaultInfo,
    biometric: impl Fn(&ContextEntry) -> bool,
) -> Vec<ContextInfo> {
    reg.contexts
        .iter()
        .map(|c| {
            let v = vault(c);
            ContextInfo {
                id: c.id.clone(),
                label: c.label.clone(),
                kind: c.kind.clone(),
                path: c.path.clone(),
                server_url: c.server_url.clone(),
                workspace_id: c.workspace_id.clone(),
                active: c.id == reg.active_id,
                vault_exists: v.exists,
                vault_biometric: biometric(c),
                vault_generation: v.generation,
                vault_rotation_pending: v.rotation_pending,
                role: v.role.clone(),
                invites_needing_code: v.invites_needing_code,
            }
        })
        .collect()
}

/// Thin wrapper around [`to_infos_with`] for callers that don't need the
/// (comparatively expensive) vault flags — every context-mutation op that
/// returns a fresh snapshot after `add`/`rename`/`remove`/etc.
pub fn to_infos(reg: &Registry) -> Vec<ContextInfo> {
    to_infos_with(reg, |_| ContextVaultInfo::default(), |_| false)
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
            vault_rotation_pending: false,
        }),
        Some(ctx) if ctx.workspace_id.is_empty() => Some(SyncStatus {
            state: "unbound".into(),
            last_synced_at: 0,
            pending: 0,
            vault_rotation_pending: false,
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
        // Read here rather than by re-listing every context from the
        // frontend on every pull — the badge that shows it lives next to the
        // sync state anyway.
        vault_rotation_pending: crate::migrate::get_meta_i64(
            &store.conn,
            "vault_rotation_pending",
            0,
        ) != 0,
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
/// rows that were pushed and are still unchanged, merge the pulled rows,
/// cache/flag the workspace vault-key state the server sent along, and
/// advance both sync markers. `note_ids` / `folder_ids` are the snapshots
/// from the matching [`SyncPush`].
pub fn commit_sync_result(
    store: &Store,
    note_ids: &[(String, i64)],
    folder_ids: &[(String, i64)],
    pull: &crate::sync::PullBody,
    now: i64,
) -> Result<(), String> {
    store
        .clear_note_dirty(note_ids)
        .map_err(|e| e.to_string())?;
    crate::folders::clear_folder_dirty(&store.conn, folder_ids).map_err(|e| e.to_string())?;
    crate::sync::apply_pulled(store, &pull.folders, &pull.notes).map_err(|e| e.to_string())?;
    apply_vault_keys(store, pull)?;
    crate::migrate::set_meta_i64(&store.conn, "sync_cursor", pull.cursor)
        .map_err(|e| e.to_string())?;
    crate::migrate::set_meta_i64(&store.conn, "sync_last_at", now).map_err(|e| e.to_string())?;
    Ok(())
}

/// Cache the caller's own wrapped vault-key entries the server sent back on
/// pull, and record the workspace's key generation / rotation state. A
/// server that predates the vault-keys feature simply omits `vaultKeys`
/// entirely — that's recorded as `vault_server_legacy` so the UI can explain
/// why protected notes can't be shared to another device yet, and the
/// existing cache (if any, from a prior server that did support it) is left
/// untouched rather than being wiped by a downgrade.
pub fn apply_vault_keys(store: &Store, pull: &crate::sync::PullBody) -> Result<(), String> {
    match &pull.vault_keys {
        Some(keys) => {
            store
                .set_vault_entries(&keys.to_string())
                .map_err(|e| e.to_string())?;
            crate::migrate::set_meta_i64(
                &store.conn,
                "vault_generation",
                i64::from(pull.vault_generation.unwrap_or(0)),
            )
            .map_err(|e| e.to_string())?;
            crate::migrate::set_meta_i64(
                &store.conn,
                "vault_rotation_pending",
                i64::from(pull.vault_rotation_pending),
            )
            .map_err(|e| e.to_string())?;
            if let Some(role) = &pull.workspace_role {
                crate::migrate::set_meta(&store.conn, "workspace_role", role)
                    .map_err(|e| e.to_string())?;
            }
            if let Some(invites) = &pull.vault_invites {
                crate::migrate::set_meta(&store.conn, "vault_invites", &invites.to_string())
                    .map_err(|e| e.to_string())?;
            }
            crate::migrate::delete_meta(&store.conn, "vault_server_legacy")
                .map_err(|e| e.to_string())
        }
        None => crate::migrate::set_meta_i64(&store.conn, "vault_server_legacy", 1)
            .map_err(|e| e.to_string()),
    }
}

/// Invitation ids whose vault wrap is missing or older than `generation` —
/// the owner has to hand out a fresh code for each. Unparsable input → empty.
///
/// `generation == 0` also → empty: that's a context with no vault at all (a
/// local context, or a server one that has never pulled `vaultGeneration`),
/// and every invitation would otherwise look "stale" against it (an absent
/// `generation` on an item passes the `< generation` check vacuously for any
/// `generation`, generation 0 included) — a vaultless context must never
/// claim invitations need codes.
pub fn invites_needing_code(invites_json: &str, generation: u32) -> Vec<u64> {
    if generation == 0 {
        return Vec::new();
    }
    let Ok(serde_json::Value::Array(items)) =
        serde_json::from_str::<serde_json::Value>(invites_json)
    else {
        return Vec::new();
    };
    items
        .iter()
        .filter(|i| {
            i["generation"]
                .as_u64()
                .is_none_or(|g| (g as u32) < generation)
        })
        .filter_map(|i| i["invitationId"].as_u64())
        .collect()
}

/// The open invitations (owner's cache, see `apply_vault_keys`) whose wrap
/// is missing or predates the current generation.
pub fn recode_targets(store: &Store) -> Result<Vec<u64>, String> {
    let generation = u32::try_from(crate::migrate::get_meta_i64(
        &store.conn,
        "vault_generation",
        0,
    ))
    .unwrap_or(0);
    Ok(crate::migrate::get_meta(&store.conn, "vault_invites")
        .map_err(|e| e.to_string())?
        .map(|json| invites_needing_code(&json, generation))
        .unwrap_or_default())
}

/// The cached invitation list with `ids` now carrying `generation` — so the
/// badge clears immediately instead of waiting for the next pull.
pub fn mark_invites_recoded(invites_json: &str, ids: &[u64], generation: u32) -> String {
    let Ok(serde_json::Value::Array(mut items)) =
        serde_json::from_str::<serde_json::Value>(invites_json)
    else {
        return invites_json.to_string();
    };
    for item in items.iter_mut() {
        if item["invitationId"]
            .as_u64()
            .is_some_and(|id| ids.contains(&id))
        {
            item["generation"] = serde_json::Value::from(generation);
        }
    }
    serde_json::Value::Array(items).to_string()
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

    /// A `VaultState` holding exactly one generation — the shape almost
    /// every op test needs. Lived as four identical copies across the test
    /// modules before.
    pub fn unlocked_at(generation: u32, dek: Dek) -> VaultState {
        let mut v = VaultState::default();
        v.unlock(generation, dek);
        v
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

    /// The persisted vault record, as the commands load it before running the
    /// KDF outside the store lock.
    pub fn record(s: &Store) -> crate::vault::VaultRecord {
        super::load_vault_record(s).unwrap()
    }

    /// `Result::unwrap_err` needs `T: Debug`, which `Dek`/`VaultRecord`
    /// deliberately do not implement (a Debug impl on key material is exactly
    /// what must never exist). This gets at the error without that bound.
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

        encrypt_note_in_place(&s, "n1", &dek, 1).unwrap();

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

        encrypt_note_in_place(&s, "n1", &dek, 1).unwrap();

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

        encrypt_note_in_place(&s, "a", &dek, 1).unwrap();

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

        encrypt_note_in_place(&s, "n1", &dek, 1).unwrap();

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

        encrypt_note_in_place(&s, "n1", &dek, 1).unwrap();

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
        encrypt_note_in_place(&s, "ghost", &dek, 1).unwrap();
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

        backfill_protected_titles(&s, &unlocked_at(1, dek));

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

        backfill_protected_titles(&s, &unlocked_at(1, dek));

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
        backfill_protected_titles(&s, &unlocked_at(1, wrong));

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

        backfill_protected_titles(&s, &unlocked_at(1, dek));

        assert_eq!(title_of(&s, "n1"), "Handpicked");
    }

    #[test]
    fn ignores_unprotected_notes_entirely() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>plain</p>"); // `save_note` leaves `title` empty

        backfill_protected_titles(&s, &unlocked_at(1, dek));

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
        backfill_protected_titles(&s, &unlocked_at(1, Dek::random()));
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
        assert_eq!(
            open_note_content(&s, &VaultState::default(), "ghost").unwrap(),
            ""
        );
    }

    #[test]
    fn plaintext_note_reads_back_verbatim_without_a_dek() {
        let s = store();
        seed(&s, "n1", "<p>hello</p>");
        assert_eq!(
            open_note_content(&s, &VaultState::default(), "n1").unwrap(),
            "<p>hello</p>"
        );
    }

    #[test]
    fn protected_note_is_decrypted_when_the_vault_is_unlocked() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>secret</p>");
        encrypt_note_in_place(&s, "n1", &dek, 1).unwrap();

        assert_eq!(
            open_note_content(&s, &unlocked_at(1, dek), "n1").unwrap(),
            "<p>secret</p>"
        );
    }

    #[test]
    fn protected_note_is_refused_while_the_vault_is_locked() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>secret</p>");
        encrypt_note_in_place(&s, "n1", &dek, 1).unwrap();

        assert_eq!(
            open_note_content(&s, &VaultState::default(), "n1").unwrap_err(),
            "vault locked"
        );
    }

    #[test]
    fn protected_note_under_a_foreign_dek_errors_instead_of_leaking_ciphertext() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>secret</p>");
        encrypt_note_in_place(&s, "n1", &dek, 1).unwrap();

        // The vault is unlocked with the SAME generation (1) but a DIFFERENT
        // (foreign) DEK — e.g. a key from another context — so the ring has
        // an entry for the note's generation and `open_content` runs, but
        // fails to authenticate.
        let err = open_note_content(&s, &unlocked_at(1, Dek::random()), "n1").unwrap_err();
        assert!(!err.contains("secret"));
        assert!(!err.is_empty());
    }

    /// A row whose `key_gen` disagrees with the key its bytes were sealed
    /// with must still open when the right DEK IS in the ring — the stamp is
    /// a hint, the AEAD tag is the proof.
    #[test]
    fn a_mis_stamped_note_still_opens_under_whichever_generation_actually_seals_it() {
        let s = store();
        let (d1, d2) = (Dek::random(), Dek::random());
        seed(&s, "n1", "<p>secret</p>");
        encrypt_note_in_place(&s, "n1", &d2, 2).unwrap();
        // Corrupt only the stamp, never the bytes.
        s.set_note_key_gen("n1", Some(1)).unwrap();

        let mut ring = VaultState::default();
        ring.unlock(1, d1);
        ring.unlock(2, d2);
        assert_eq!(open_note_content(&s, &ring, "n1").unwrap(), "<p>secret</p>");

        // A ring without generation 2 still cannot open it, and says so
        // without leaking anything.
        let err = open_note_content(&s, &unlocked_at(1, Dek::random()), "n1").unwrap_err();
        assert!(!err.contains("secret") && !err.is_empty());
    }

    #[test]
    fn protected_note_is_refused_when_its_generation_is_not_in_the_ring() {
        // The vault is unlocked, but only at a generation OTHER than the one
        // this note was sealed under — distinct from both "vault locked" and
        // a foreign-key decrypt failure.
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>secret</p>");
        encrypt_note_in_place(&s, "n1", &dek, 1).unwrap();

        let err = open_note_content(&s, &unlocked_at(2, Dek::random()), "n1").unwrap_err();
        assert_eq!(err, "key generation not available");
    }

    #[test]
    fn a_note_in_a_locked_folder_that_is_still_plaintext_reads_back_plainly() {
        // `protected` — not the folder flag — is what gates decryption on read,
        // matching the "content is ciphertext iff protected = 1" invariant.
        let s = store();
        folder(&s, "f", None);
        seed_in(&s, "n1", "<p>plain</p>", "f");
        s.set_folder_locked("f", true).unwrap();

        assert_eq!(
            open_note_content(&s, &VaultState::default(), "n1").unwrap(),
            "<p>plain</p>"
        );
    }
}

#[cfg(test)]
mod key_ring_tests {
    use super::test_support::*;
    use super::*;

    /// End-to-end across `VaultState`'s ring and the `ops` sealing/opening
    /// surface: a note sealed while generation 1 was the newest keeps opening
    /// under generation 1 even after the vault rotates to generation 2 and a
    /// NEW note starts sealing under that newest generation instead.
    #[test]
    fn notes_seal_with_the_newest_generation_and_open_with_their_own() {
        let s = Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        let (d1, d2) = (Dek::random(), Dek::random());
        let mut vault = VaultState::default();
        vault.unlock(1, d1.clone());
        s.save_note(&Note {
            id: "old".into(),
            content: "<p>old</p>".into(),
            updated_at: 1,
            ..Default::default()
        })
        .unwrap();
        s.set_note_protected("old", true).unwrap();
        encrypt_note_in_place(&s, "old", &d1, 1).unwrap();
        assert_eq!(s.note_key_gen("old").unwrap(), Some(1));

        vault.unlock(2, d2.clone());
        let mut fresh = Note {
            id: "new".into(),
            content: "<p>new</p>".into(),
            updated_at: 2,
            ..Default::default()
        };
        s.save_note(&fresh).unwrap();
        s.set_note_protected("new", true).unwrap();
        fresh.content = "<p>new v2</p>".into();
        save_note(
            &s,
            Some((vault.dek().unwrap(), vault.newest_generation().unwrap())),
            &fresh,
        )
        .unwrap();
        assert_eq!(s.note_key_gen("new").unwrap(), Some(2));

        assert_eq!(open_note_content(&s, &vault, "old").unwrap(), "<p>old</p>");
        assert_eq!(
            open_note_content(&s, &vault, "new").unwrap(),
            "<p>new v2</p>"
        );
        let mut only_new = VaultState::default();
        only_new.unlock(2, d2);
        assert_eq!(
            open_note_content(&s, &only_new, "old").unwrap_err(),
            "key generation not available"
        );
        assert_eq!(
            open_note_content(&s, &VaultState::default(), "new").unwrap_err(),
            "vault locked"
        );
    }

    /// R2: a device whose ring is behind the workspace's generation has not
    /// redeemed its rotation code. It must not seal anything new under its
    /// stale key — that key is exactly the one the rotation locked out.
    /// Reading and unsealing stay allowed.
    #[test]
    fn an_outdated_ring_may_not_seal_new_content() {
        let s = store();
        let d1 = Dek::random();
        let vault = unlocked_at(1, d1.clone());
        seed(&s, "n1", "<p>plain</p>");
        crate::folders::create_folder(&s.conn, "f", "F", None).unwrap();
        // Sealed while this device was still up to date.
        seed(&s, "n2", "<p>x</p>");
        encrypt_note_in_place(&s, "n2", &d1, 1).unwrap();
        // The last pull says the workspace is on generation 2; the ring is on 1.
        crate::migrate::set_meta_i64(&s.conn, "vault_generation", 2).unwrap();

        let outdated = "vault: key generation outdated — unlock with your passphrase";
        assert_eq!(
            set_note_protected(&s, &vault, "n1", true).unwrap_err(),
            outdated
        );
        assert_eq!(
            set_folder_locked(&s, &vault, "f", true).unwrap_err(),
            outdated
        );
        assert!(!s.note_protected("n1").unwrap());
        assert!(!s.folder_locked("f").unwrap());

        // Editing an ALREADY protected note is refused for the same reason.
        assert_eq!(
            save_note(&s, Some((&d1, 1)), &note("n2", "<p>edited</p>")).unwrap_err(),
            outdated
        );
        assert_eq!(
            open_note_content(&s, &vault, "n2").unwrap(),
            "<p>x</p>",
            "the refused edit wrote nothing"
        );
        // A plaintext note is untouched by the rule — nothing is being sealed.
        save_note(&s, Some((&d1, 1)), &note("n3", "<p>plain</p>")).unwrap();

        // Caught up (the code was redeemed): sealing works again.
        let mut ring = VaultState::default();
        ring.unlock(1, d1.clone());
        ring.unlock(2, Dek::random());
        set_note_protected(&s, &ring, "n1", true).unwrap();
        assert_eq!(s.note_key_gen("n1").unwrap(), Some(2));
    }

    /// Round 2 / Important 2: on a CONFLICTED device the ring is that
    /// device's own vault, whose generation numbering has nothing to do with
    /// the workspace's. Comparing them would lock the user out of their own
    /// notes with advice ("unlock with your passphrase") that cannot help.
    #[test]
    fn a_conflicted_device_keeps_sealing_under_its_own_vault() {
        let s = store();
        let d1 = Dek::random();
        let vault = unlocked_at(1, d1.clone());
        seed(&s, "n1", "<p>mine</p>");
        crate::folders::create_folder(&s.conn, "f", "F", None).unwrap();
        // The workspace is far ahead; this device holds its OWN generation 1.
        crate::migrate::set_meta_i64(&s.conn, "vault_generation", 3).unwrap();
        crate::migrate::set_meta_i64(&s.conn, "vault_conflict", 1).unwrap();

        set_note_protected(&s, &vault, "n1", true).unwrap();
        assert_eq!(s.note_key_gen("n1").unwrap(), Some(1));
        save_note(&s, Some((&d1, 1)), &note("n1", "<p>edited</p>")).unwrap();
        assert_eq!(
            open_note_content(&s, &vault, "n1").unwrap(),
            "<p>edited</p>"
        );
        set_folder_locked(&s, &vault, "f", true).unwrap();
        assert!(s.folder_locked("f").unwrap());

        // Resolve the conflict (an unlock proved the two are one vault) and
        // the ordinary rule applies again.
        crate::migrate::delete_meta(&s.conn, "vault_conflict").unwrap();
        seed(&s, "n2", "<p>plain</p>");
        assert_eq!(
            set_note_protected(&s, &vault, "n2", true).unwrap_err(),
            "vault: key generation outdated — unlock with your passphrase"
        );
    }

    /// Round 3 / minor 1: the exemption is only for a device sealing with its
    /// OWN vault. A conflicted device can just as well hold WORKSPACE
    /// generations — the unlock fallback installs the workspace ring when the
    /// entries open, and a redeemed rotation installs them outright — and
    /// those are exactly the keys a rotation was meant to retire.
    #[test]
    fn a_conflicted_device_holding_a_workspace_key_is_still_guarded() {
        let s = store();
        let workspace_dek = Dek::random();
        // The workspace handed this caller generation 1 and has since rotated.
        s.set_vault_entries(
            &VaultEntries {
                mine: vec![MyEntry::try_from(my_entry_for(&workspace_dek, 1, "pw")).unwrap()],
                recovery: vec![],
                rotation: vec![],
            }
            .to_json(),
        )
        .unwrap();
        crate::migrate::set_meta_i64(&s.conn, "vault_generation", 2).unwrap();
        crate::migrate::set_meta_i64(&s.conn, "vault_conflict", 1).unwrap();
        seed(&s, "n1", "<p>plain</p>");

        // Sealing with the WORKSPACE's generation 1: refused, conflict or not.
        assert_eq!(
            set_note_protected(&s, &unlocked_at(1, workspace_dek.clone()), "n1", true).unwrap_err(),
            "vault: key generation outdated — unlock with your passphrase"
        );
        assert!(!s.note_protected("n1").unwrap());

        // The very same generation NUMBER, but this device's own key: exempt.
        set_note_protected(&s, &unlocked_at(1, Dek::random()), "n1", true).unwrap();
        assert!(s.note_protected("n1").unwrap());

        // And the UI mirrors both answers.
        let flags = |dek: &Dek| {
            let f = vault_status_flags(&s, true, Some((1, dek))).unwrap();
            seal_outdated(
                f.server_generation,
                Some(1),
                true,
                f.conflict,
                f.ring_is_workspace,
            )
        };
        assert!(flags(&workspace_dek), "read-only: a stale workspace key");
        assert!(!flags(&Dek::random()), "editable: this device's own vault");
    }

    /// Round 2 / minor: dragging a plaintext note into a locked folder is a
    /// seal like any other and must refuse the same way.
    #[test]
    fn moving_a_note_into_a_locked_folder_refuses_on_an_outdated_ring() {
        // A SYNCING store, so the "was it queued for the server?" half of
        // this is real: `Store::set_folder` marks the row dirty.
        let s = syncing_store();
        let d1 = Dek::random();
        crate::folders::create_folder(&s.conn, "locked", "L", None).unwrap();
        seed(&s, "n1", "<p>plain</p>");
        set_folder_locked(&s, &unlocked_at(1, d1.clone()), "locked", true).unwrap();
        seed(&s, "n2", "<p>plain</p>");
        clear_dirty(&s);
        crate::migrate::set_meta_i64(&s.conn, "vault_generation", 2).unwrap();

        let folder_of = |id: &str| -> Option<String> {
            s.conn
                .query_row("SELECT folder_id FROM notes WHERE id = ?1", [id], |r| {
                    r.get::<_, Option<String>>(0)
                })
                .unwrap()
        };
        assert_eq!(folder_of("n2"), None, "starts at the root");

        let outdated = "vault: key generation outdated — unlock with your passphrase";
        assert_eq!(
            reconcile_folder_move(&s, "n2", Some("locked"), Some((&d1, 1))).unwrap_err(),
            outdated
        );
        // Refused BEFORE the move is committed: a note that had already been
        // reparented would sit inside the locked subtree as plaintext — and
        // be pushed to the workspace exactly like that.
        assert_eq!(folder_of("n2"), None, "the move never happened");
        assert!(
            !s.note_protected("n2").unwrap(),
            "and it is still plaintext"
        );

        assert_eq!(
            reconcile_reorder(&s, Some("locked"), &["n2".to_string()], Some((&d1, 1))).unwrap_err(),
            outdated
        );
        assert_eq!(folder_of("n2"), None, "the reorder never happened");
        assert!(!s.note_protected("n2").unwrap());
        assert!(
            s.load_dirty_notes().unwrap().is_empty(),
            "a refused move must not queue the plaintext row for the server"
        );

        // Caught up: the same move goes through and seals on arrival.
        let mut ring = VaultState::default();
        ring.unlock(1, d1.clone());
        ring.unlock(2, Dek::random());
        let newest = (ring.dek().unwrap(), ring.newest_generation().unwrap());
        reconcile_folder_move(&s, "n2", Some("locked"), Some(newest)).unwrap();
        assert_eq!(folder_of("n2").as_deref(), Some("locked"));
        assert!(s.note_protected("n2").unwrap());
    }

    /// R5: the generation stamp rides along in the SAME statement that writes
    /// the ciphertext, so no window exists in which the row holds generation
    /// N's bytes under a NULL (⇒ generation 1) or stale stamp.
    #[test]
    fn a_protected_save_stamps_its_generation_with_the_ciphertext() {
        let s = store();
        let d3 = Dek::random();
        let mut vault = VaultState::default();
        vault.unlock(3, d3.clone());
        seed(&s, "n1", "<p>plain</p>");
        s.set_note_protected("n1", true).unwrap();

        let note = Note {
            id: "n1".into(),
            content: "<p>secret</p>".into(),
            updated_at: 5,
            // Exactly what the frontend sends: no generation of its own.
            ..Default::default()
        };
        save_note(&s, Some((&d3, 3)), &note).unwrap();

        assert_eq!(s.note_key_gen("n1").unwrap(), Some(3));
        assert_eq!(
            open_note_content(&s, &vault, "n1").unwrap(),
            "<p>secret</p>"
        );
    }

    /// R5: a statement failing partway through a seal must leave NOTHING
    /// behind — not ciphertext with no `protected` flag, and not a stamp
    /// without the matching bytes. Provoked by removing the revisions table
    /// the transaction's last write touches.
    #[test]
    fn a_failing_seal_rolls_back_completely() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>plain</p>");
        s.conn.execute_batch("DROP TABLE note_revisions").unwrap();

        assert!(encrypt_note_in_place(&s, "n1", &dek, 2).is_err());

        assert!(!s.note_protected("n1").unwrap(), "flag rolled back");
        assert_eq!(s.note_key_gen("n1").unwrap(), None, "stamp rolled back");
        assert_eq!(
            s.load_note_content("n1").unwrap().unwrap(),
            "<p>plain</p>",
            "content rolled back — never ciphertext without its flag"
        );
    }

    /// R5: the same guarantee for the lazy sweep — a note is never left with
    /// one generation's ciphertext under another generation's stamp.
    #[test]
    fn a_resealed_note_always_opens_under_the_stamp_it_carries() {
        let s = store();
        let (d1, d2) = (Dek::random(), Dek::random());
        seed(&s, "n1", "<p>secret</p>");
        encrypt_note_in_place(&s, "n1", &d1, 1).unwrap();

        let mut ring = VaultState::default();
        ring.unlock(1, d1);
        ring.unlock(2, d2.clone());
        assert_eq!(reseal_lagging_notes(&s, &ring, 10).unwrap(), 1);

        let stamp = s.note_key_gen("n1").unwrap();
        assert_eq!(stamp, Some(2));
        // Opened with the stamp's OWN key, not the ring's convenience.
        let stored = s.load_note_content("n1").unwrap().unwrap();
        assert_eq!(open_content(&d2, "n1", &stored).unwrap(), "<p>secret</p>");
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
        encrypt_note_in_place(&s, "n1", &dek, 1).unwrap();

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
        encrypt_note_in_place(&s, "n1", &dek, 1).unwrap();

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
            encrypt_note_in_place(&s, "n1", &dek, 1).unwrap();
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

        save_note(
            &s,
            Some((&dek, 1)),
            &note("n1", "<p>Very Secret</p><p>body</p>"),
        )
        .unwrap();

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

        save_note(&s, Some((&dek, 1)), &note("n1", "<p>new secret</p>")).unwrap();

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

        save_note(&s, Some((&dek, 1)), &n).unwrap();

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

    /// Regression for the Task 9→10 handoff: `Store::save_note`'s `ON
    /// CONFLICT` clause writes `key_gen` straight from the incoming `Note`
    /// (see its `INSERT ... ON CONFLICT` in `storage.rs`), and a `Note`
    /// arriving from the frontend — or built with `..Default::default()`,
    /// like the `note()` test helper — always carries `key_gen: None`. If
    /// `save_note` didn't call `set_note_key_gen` AFTER `store.save_note`,
    /// re-saving a note sealed at an OLDER generation than the ring's newest
    /// would silently clobber its `key_gen` to `NULL`, and a later
    /// `open_note_content` would then reach for the WRONG (newest) DEK.
    #[test]
    fn resaving_a_protected_note_keeps_the_generation_it_was_sealed_under() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>v1</p>");
        s.set_note_protected("n1", true).unwrap();
        // Sealed at generation 2 — NOT the incoming `Note`'s (default) None.
        save_note(&s, Some((&dek, 2)), &note("n1", "<p>v1</p>")).unwrap();
        assert_eq!(s.note_key_gen("n1").unwrap(), Some(2));

        // Re-save through a fresh `Note` (key_gen: None, per `..Default::default()`)
        // with changed content, still sealing under the SAME generation.
        let mut resaved = note("n1", "<p>v2</p>");
        assert_eq!(
            resaved.key_gen, None,
            "the incoming Note carries no generation"
        );
        resaved.updated_at = 2;
        save_note(&s, Some((&dek, 2)), &resaved).unwrap();

        assert_eq!(
            s.note_key_gen("n1").unwrap(),
            Some(2),
            "the generation must survive the re-save, not be clobbered back to NULL"
        );
        assert_eq!(
            open_content(&dek, "n1", &content_of(&s, "n1")).unwrap(),
            "<p>v2</p>"
        );
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

        reconcile_folder_move(&s, "n1", Some("locked-folder"), Some((&dek, 1))).unwrap();

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

        reconcile_folder_move(&s, "n1", Some("sub"), Some((&dek, 1))).unwrap();

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
        encrypt_note_in_place(&s, "n1", &dek, 1).unwrap();
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
        encrypt_note_in_place(&s, "n1", &dek, 1).unwrap();
        let sealed = content_of(&s, "n1");

        reconcile_folder_move(&s, "n1", None, Some((&dek, 1))).unwrap();

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

        reconcile_reorder(&s, Some("locked-folder"), &["n1".into()], Some((&dek, 1))).unwrap();

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
        encrypt_note_in_place(&s, "already", &Dek::random(), 1).unwrap();

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
        encrypt_note_in_place(&s, "already", &dek, 1).unwrap();
        let already_sealed = content_of(&s, "already");

        reconcile_reorder(
            &s,
            Some("locked-folder"),
            &["already".into(), "plain".into()],
            Some((&dek, 1)),
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
        encrypt_note_in_place(&s, "a", &Dek::random(), 1).unwrap();

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
            Some((&dek, 1)),
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
        encrypt_note_in_place(&s, "n1", &dek, 1).unwrap();

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
        encrypt_note_in_place(&s, "n1", &dek, 1).unwrap();

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

/// The workspace-vault entry cache: parsing, unlocking every generation it
/// carries, and the payloads that put a local vault onto the server.
#[cfg(test)]
mod vault_entries_tests {
    use super::test_support::*;
    use super::*;

    /// A `MyEntry` for `generation` built from a full record.
    fn mine(generation: u32, record: VaultRecord) -> MyEntry {
        MyEntry { generation, record }
    }

    /// The `recovery` half of a record, as the server would hand it back.
    fn recovery_of(generation: u32, rec: &VaultRecord) -> RecoveryEntry {
        RecoveryEntry {
            generation,
            recovery_salt: rec.recovery_salt,
            dek_wrapped_recovery: rec.dek_wrapped_recovery.clone(),
            dek_check: rec.dek_check.clone(),
        }
    }

    /// R4: an entry whose KDF parameters are out of range is unopenable, not
    /// an invitation to allocate a gigabyte inside an unlock.
    #[test]
    fn an_entry_with_absurd_kdf_parameters_is_refused() {
        let dek = Dek::random();
        let mut wire = my_entry_for(&dek, 1, "pw");
        wire.kdf_params.m_cost = u32::MAX;
        assert!(err_of(MyEntry::try_from(wire.clone())).contains("key-derivation parameters"));

        wire.kdf_params.m_cost = 19_456;
        wire.kdf_params.t_cost = 0;
        assert!(MyEntry::try_from(wire).is_err());

        // A whole cache is only as good as its entries: one bad one makes the
        // cache unusable, and the unlock falls back to the local record.
        let s = store();
        s.set_vault_entries(
            &serde_json::json!({
                "mine": [{
                    "generation": 1,
                    "kdfParams": { "salt": vec![0u8; 16], "mCost": u32::MAX, "tCost": 2, "pCost": 1 },
                    "dekWrapped": "",
                    "dekCheck": "",
                }],
                "recovery": [],
                "rotation": [],
            })
            .to_string(),
        )
        .unwrap();
        assert!(cached_vault_entries(&s).unwrap().is_none());
    }

    /// R4: an invite wrap is server-supplied too, and its refusal must stay
    /// indistinguishable from "wrong code".
    #[test]
    fn an_invite_wrap_with_absurd_kdf_parameters_is_refused_as_invalid() {
        let dek = Dek::random();
        let (code, mut wrap) = make_invite_wrap(&dek, 1);
        wrap.kdf_params.m_cost = u32::MAX;
        assert_eq!(
            err_of(open_invite_wrap(&wrap, &code)),
            "invalid invite code"
        );
    }

    /// R4: one unlock derives at most 32 KEKs, keeping the NEWEST generations
    /// — a server cannot turn a single unlock into minutes of Argon2.
    #[test]
    fn an_unlock_only_ever_tries_the_newest_thirty_two_generations() {
        let gens: Vec<u32> = (1..=100).collect();
        let kept = newest_generations(&gens, |g| *g);
        assert_eq!(kept.len(), MAX_UNLOCK_GENERATIONS);
        assert_eq!(*kept[0], 69, "the newest 32 of 1..=100");
        assert_eq!(*kept[31], 100);
        assert!(
            kept.windows(2).all(|w| w[0] < w[1]),
            "handed back ascending"
        );

        // A short list is kept whole, and the server's ordering is not trusted.
        let jumbled = vec![3u32, 1, 2];
        assert_eq!(
            newest_generations(&jumbled, |g| *g)
                .into_iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    /// R2: the biometric gate has to verify against the source that covers
    /// the generation the keychain item was enrolled at — the local record
    /// only ever mirrors generation 1.
    #[test]
    fn the_biometric_gate_verifies_against_the_generation_it_was_enrolled_at() {
        let s = store();
        let (rec1, _rk, d1) = crate::vault::setup("pw").unwrap();
        s.set_vault_record(&rec1.to_json()).unwrap();

        // Generation 1: the local record is enough.
        assert!(verify_dek_for_store(&s, 1, &d1).is_ok());
        assert!(err_of(verify_dek_for_store(&s, 1, &Dek::random())).contains("different context"));

        // Generation 2 has no source at all yet — refused, never guessed.
        let d2 = Dek::random();
        assert!(err_of(verify_dek_for_store(&s, 2, &d2)).contains("unlock with your passphrase"));

        // Once the workspace's wrap for generation 2 is cached, it is the
        // source — and generation 1 still answers from its own entry.
        let entries = VaultEntries {
            mine: vec![
                MyEntry::try_from(my_entry_for(&d1, 1, "pw")).unwrap(),
                MyEntry::try_from(my_entry_for(&d2, 2, "pw")).unwrap(),
            ],
            recovery: vec![],
            rotation: vec![],
        };
        s.set_vault_entries(&entries.to_json()).unwrap();
        assert!(verify_dek_for_store(&s, 2, &d2).is_ok());
        assert!(verify_dek_for_store(&s, 1, &d1).is_ok());
        assert!(err_of(verify_dek_for_store(&s, 2, &d1)).contains("different context"));
    }

    /// A member who joined at generation 2 mirrors no local record at all —
    /// the cache is their only verification source, and generation 1 (which
    /// they never held) must not fall through to a missing record.
    #[test]
    fn the_biometric_gate_works_without_a_local_record() {
        let s = store();
        let d2 = Dek::random();
        let entries = VaultEntries {
            mine: vec![MyEntry::try_from(my_entry_for(&d2, 2, "pw")).unwrap()],
            recovery: vec![],
            rotation: vec![],
        };
        s.set_vault_entries(&entries.to_json()).unwrap();
        assert!(verify_dek_for_store(&s, 2, &d2).is_ok());
        assert!(err_of(verify_dek_for_store(&s, 1, &d2)).contains("not set up"));
    }

    /// An entry that predates `dek_check` proves nothing; the DEK must not be
    /// accepted on the strength of "nothing said no".
    #[test]
    fn the_biometric_gate_refuses_an_entry_with_no_check() {
        let s = store();
        let (rec, _rk, dek) = crate::vault::setup("pw").unwrap();
        let mut checkless = rec.clone();
        checkless.dek_check = None;
        s.set_vault_record(&checkless.to_json()).unwrap();
        assert!(err_of(verify_dek_for_store(&s, 1, &dek)).contains("unlock with your passphrase"));
    }

    /// A store with a local generation-1 record and a cached generation-2
    /// `mine` entry — the shape `verify_ring_for_store`'s ring tests share.
    fn store_with_gen1_and_gen2(d1: &Dek, d2: &Dek) -> Store {
        let s = store();
        let (rec1, _rk, _dek) = crate::vault::setup("pw").unwrap();
        // `crate::vault::setup` mints its own random DEK; rewrap the record
        // under the caller's `d1` so generation 1 verifies against exactly
        // that key, matching the fixture the other biometric-gate tests use.
        let rec1 = crate::vault::rewrap_passphrase(&rec1, d1, "pw");
        s.set_vault_record(&rec1.to_json()).unwrap();
        let entries = VaultEntries {
            mine: vec![MyEntry::try_from(my_entry_for(d2, 2, "pw")).unwrap()],
            recovery: vec![],
            rotation: vec![],
        };
        s.set_vault_entries(&entries.to_json()).unwrap();
        s
    }

    /// R3: `verify_ring_for_store` partitions the ring instead of failing it
    /// whole — a generation with no source that covers it is rejected with
    /// "different context" (a foreign key against a real, checked record),
    /// while a generation that does verify is kept.
    #[test]
    fn verify_ring_for_store_partitions_verified_from_rejected() {
        let (d1, d2) = (Dek::random(), Dek::random());
        let s = store_with_gen1_and_gen2(&d1, &d2);
        let foreign = Dek::random();
        let (verified, rejected) = verify_ring_for_store(&s, &[(1, foreign), (2, d2.clone())]);
        assert_eq!(
            verified.iter().map(|(g, _)| *g).collect::<Vec<_>>(),
            vec![2]
        );
        assert_eq!(verified[0].1.expose(), d2.expose());
        assert_eq!(rejected.len(), 1);
        assert_eq!(rejected[0].0, 1);
        assert!(rejected[0].1.contains("different context"));
    }

    #[test]
    fn verify_ring_for_store_keeps_everything_when_every_generation_verifies() {
        let (d1, d2) = (Dek::random(), Dek::random());
        let s = store_with_gen1_and_gen2(&d1, &d2);
        let (verified, rejected) = verify_ring_for_store(&s, &[(1, d1.clone()), (2, d2.clone())]);
        assert_eq!(
            verified.iter().map(|(g, _)| *g).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(verified[0].1.expose(), d1.expose());
        assert_eq!(verified[1].1.expose(), d2.expose());
        assert!(rejected.is_empty());
    }

    #[test]
    fn verify_ring_for_store_rejects_every_generation_when_none_verify() {
        let (d1, d2) = (Dek::random(), Dek::random());
        let s = store_with_gen1_and_gen2(&d1, &d2);
        let (verified, rejected) =
            verify_ring_for_store(&s, &[(1, Dek::random()), (2, Dek::random())]);
        assert!(verified.is_empty());
        assert_eq!(
            rejected.iter().map(|(g, _)| *g).collect::<Vec<_>>(),
            vec![1, 2]
        );
    }

    #[test]
    fn entries_unlock_every_generation_the_passphrase_opens() {
        let (rec1, _rk, d1) = crate::vault::setup("pw").unwrap();
        let rec2 = crate::vault::rewrap_passphrase(
            &crate::vault::setup("pw").unwrap().0,
            &Dek::random(),
            "pw",
        );
        let entries = VaultEntries {
            rotation: vec![],
            mine: vec![mine(1, rec1), mine(2, rec2)],
            recovery: vec![],
        };
        let json = entries.to_json();
        let back = VaultEntries::from_json(&json).unwrap();
        let opened = unlock_entries_with_passphrase(&back, "pw").unwrap();
        assert_eq!(
            opened.iter().map(|(g, _)| *g).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(opened[0].1.expose(), d1.expose());
        assert_eq!(
            err_of(unlock_entries_with_passphrase(&back, "nope")),
            "wrong passphrase"
        );
    }

    #[test]
    fn migration_payload_reuses_the_local_record_and_setup_payload_is_complete() {
        let (rec, _rk, _dek) = crate::vault::setup("pw").unwrap();
        let p = migration_payload(&rec).unwrap();
        let v = serde_json::to_value(&p).unwrap();
        for k in ["kdfParams", "dekWrapped", "dekCheck"] {
            assert!(v[k].is_object() || v[k].is_string(), "{k}");
        }
        for k in ["recoverySalt", "dekWrappedRecovery", "dekCheck"] {
            assert!(v["recovery"][k].is_string(), "{k}");
        }
        let (p2, groups, _dek2) = vault_setup_payload("pw2").unwrap();
        assert_eq!(
            groups.len(),
            crate::vault::recovery::RecoveryKey::generate()
                .as_str()
                .split('-')
                .count()
        );
        assert!(serde_json::to_value(&p2).unwrap()["dekCheck"].is_string());
    }

    #[test]
    fn migration_payload_refuses_a_record_without_a_dek_check() {
        // A pre-`dek_check` record can't prove which vault its DEK belongs to,
        // so it is skipped (not uploaded) until an unlock self-heals it.
        let (mut rec, _rk, _dek) = crate::vault::setup("pw").unwrap();
        rec.dek_check = None;
        assert!(migration_payload(&rec).is_err());
    }

    #[test]
    fn entries_roundtrip_carries_the_recovery_wrap_and_unlocks_from_it() {
        let (rec, rk, dek) = crate::vault::setup("pw").unwrap();
        let entries = VaultEntries {
            rotation: vec![],
            recovery: vec![recovery_of(1, &rec)],
            mine: vec![mine(1, rec)],
        };
        let back = VaultEntries::from_json(&entries.to_json()).unwrap();
        assert_eq!(back.recovery.len(), 1);
        let opened = unlock_entries_with_recovery(&back, rk.as_str()).unwrap();
        assert_eq!(opened.len(), 1);
        assert_eq!(opened[0].0, 1);
        assert_eq!(opened[0].1.expose(), dek.expose());
        assert_eq!(
            err_of(unlock_entries_with_recovery(&back, "AAAAA-BBBBB-CCCCC")),
            "wrong recovery key"
        );
    }

    #[test]
    fn vault_exists_is_true_from_cached_entries_without_a_local_record() {
        let s = store();
        assert!(!vault_exists(&s).unwrap());
        let (rec, _rk, _dek) = crate::vault::setup("pw").unwrap();
        let entries = VaultEntries {
            rotation: vec![],
            mine: vec![mine(1, rec)],
            recovery: vec![],
        };
        s.set_vault_entries(&entries.to_json()).unwrap();
        assert!(
            s.vault_record().unwrap().is_none(),
            "a freshly synced device has no local record yet"
        );
        assert!(
            vault_exists(&s).unwrap(),
            "such a device must be offered unlock, not setup"
        );
    }

    #[test]
    fn a_new_device_mirrors_the_generation_one_record_and_marks_itself_migrated() {
        let s = store();
        let (rec, rk, dek) = crate::vault::setup("pw").unwrap();
        let entries = VaultEntries {
            rotation: vec![],
            recovery: vec![recovery_of(1, &rec)],
            mine: vec![mine(1, rec)],
        };
        let plan = plan_entry_unlock(None, &entries, &VaultSecret::Passphrase("pw")).unwrap();
        assert!(plan.reconciled, "nothing local to disagree with");
        apply_entry_unlock(&s, None, &entries, &plan, &VaultSecret::Passphrase("pw")).unwrap();

        // The mirrored record opens with BOTH the passphrase and the recovery
        // key, so biometric enrolment and `ensure_dek_check` keep working.
        let mirrored = load_vault_record(&s).unwrap();
        assert_eq!(
            vault_unlock_passphrase(&mirrored, "pw").unwrap().expose(),
            dek.expose()
        );
        assert_eq!(
            vault_unlock_recovery(&mirrored, rk.as_str())
                .unwrap()
                .expose(),
            dek.expose()
        );
        assert_eq!(
            crate::migrate::get_meta_i64_opt(&s.conn, "vault_migrated").unwrap(),
            Some(1)
        );
    }

    /// meta helper: `None` when unset.
    fn meta(s: &Store, key: &str) -> Option<i64> {
        crate::migrate::get_meta_i64_opt(&s.conn, key).unwrap()
    }

    /// A local record plus a recorded conflict, as the sync hook leaves it.
    fn conflicted(s: &Store, rec: &VaultRecord) {
        s.set_vault_record(&rec.to_json()).unwrap();
        crate::migrate::set_meta_i64(&s.conn, "vault_conflict", 1).unwrap();
    }

    #[test]
    fn the_same_vault_on_both_sides_installs_every_generation_and_reconciles() {
        let s = store();
        let (local, _rk, dek) = crate::vault::setup("pw").unwrap();
        conflicted(&s, &local);
        // The workspace wraps the SAME DEK at generation 1 (plus a later
        // rotation), so the two are provably one vault.
        let gen2 = Dek::random();
        let entries = VaultEntries {
            rotation: vec![],
            mine: vec![
                mine(1, crate::vault::rewrap_passphrase(&local, &dek, "pw")),
                mine(2, crate::vault::rewrap_passphrase(&local, &gen2, "pw")),
            ],
            recovery: vec![],
        };
        let plan =
            plan_entry_unlock(Some(&local), &entries, &VaultSecret::Passphrase("pw")).unwrap();

        assert!(plan.reconciled);
        assert_eq!(
            plan.install.iter().map(|(g, _)| *g).collect::<Vec<_>>(),
            vec![1, 2],
            "a second device must get every generation"
        );
        assert_eq!(plan.install[0].1.expose(), dek.expose());
        apply_entry_unlock(
            &s,
            Some(&local),
            &entries,
            &plan,
            &VaultSecret::Passphrase("pw"),
        )
        .unwrap();
        assert_eq!(meta(&s, "vault_conflict"), None, "same vault: resolved");
        assert_eq!(meta(&s, "vault_migrated"), Some(1));
    }

    #[test]
    fn a_checkless_local_record_reconciles_and_heals_when_the_deks_match() {
        let s = store();
        let (mut local, _rk, dek) = crate::vault::setup("pw").unwrap();
        local.dek_check = None; // predates the check: nothing to verify against
        conflicted(&s, &local);
        let entries = VaultEntries {
            rotation: vec![],
            mine: vec![mine(1, crate::vault::rewrap_passphrase(&local, &dek, "pw"))],
            recovery: vec![],
        };
        let plan =
            plan_entry_unlock(Some(&local), &entries, &VaultSecret::Passphrase("pw")).unwrap();

        assert!(
            plan.reconciled,
            "no check to verify against -> the DEKs themselves are compared"
        );
        apply_entry_unlock(
            &s,
            Some(&local),
            &entries,
            &plan,
            &VaultSecret::Passphrase("pw"),
        )
        .unwrap();
        assert!(
            load_vault_record(&s).unwrap().dek_check.is_some(),
            "healed from a DEK proved to be this record's own"
        );
        assert_eq!(meta(&s, "vault_conflict"), None);
        assert_eq!(meta(&s, "vault_migrated"), Some(1));
    }

    #[test]
    fn a_checkless_local_record_reconciles_through_the_recovery_key_too() {
        let s = store();
        let (mut local, rk, dek) = crate::vault::setup("pw").unwrap();
        local.dek_check = None;
        conflicted(&s, &local);
        let entries = VaultEntries {
            rotation: vec![],
            mine: vec![],
            recovery: vec![recovery_of(1, &local)],
        };
        let plan =
            plan_entry_unlock(Some(&local), &entries, &VaultSecret::Recovery(rk.as_str())).unwrap();

        assert!(plan.reconciled);
        assert_eq!(plan.install[0].1.expose(), dek.expose());
        apply_entry_unlock(
            &s,
            Some(&local),
            &entries,
            &plan,
            &VaultSecret::Recovery(rk.as_str()),
        )
        .unwrap();
        assert_eq!(meta(&s, "vault_conflict"), None);
        assert_eq!(meta(&s, "vault_migrated"), Some(1));
    }

    #[test]
    fn a_foreign_workspace_vault_installs_only_the_local_key_and_keeps_the_conflict() {
        // Device A set its vault up offline; another device seeded the
        // workspace with the same passphrase but a different DEK.
        let s = store();
        let (mut local, _rk, local_dek) = crate::vault::setup("pw").unwrap();
        local.dek_check = None; // so a stray heal would be visible
        conflicted(&s, &local);
        let before = s.vault_record().unwrap().unwrap();
        let (server, _rk2, server_dek) = crate::vault::setup("pw").unwrap();
        assert_ne!(local_dek.expose(), server_dek.expose());
        let entries = VaultEntries {
            rotation: vec![],
            mine: vec![mine(1, server)],
            recovery: vec![],
        };
        let plan =
            plan_entry_unlock(Some(&local), &entries, &VaultSecret::Passphrase("pw")).unwrap();

        assert!(!plan.reconciled);
        assert_eq!(plan.install.len(), 1, "never mixed with the workspace ring");
        assert_eq!(plan.install[0].0, 1);
        assert_eq!(
            plan.install[0].1.expose(),
            local_dek.expose(),
            "this device keeps reading its OWN protected notes"
        );
        apply_entry_unlock(
            &s,
            Some(&local),
            &entries,
            &plan,
            &VaultSecret::Passphrase("pw"),
        )
        .unwrap();
        assert_eq!(
            s.vault_record().unwrap().unwrap(),
            before,
            "no heal from a foreign DEK — that would break biometric unlock"
        );
        assert!(load_vault_record(&s).unwrap().dek_check.is_none());
        assert_eq!(meta(&s, "vault_conflict"), Some(1), "still surfaced");
        assert_eq!(
            meta(&s, "vault_migrated"),
            None,
            "unclaimed, so the sync hook keeps re-marking the conflict"
        );
    }

    #[test]
    fn a_foreign_workspace_vault_falls_back_to_its_ring_when_the_local_record_stays_shut() {
        // Same as above, but the local vault has a different passphrase too,
        // so only the workspace ring can be installed.
        let s = store();
        let (local, _rk, _local_dek) = crate::vault::setup("other").unwrap();
        conflicted(&s, &local);
        let before = s.vault_record().unwrap().unwrap();
        let (server, _rk2, server_dek) = crate::vault::setup("pw").unwrap();
        let entries = VaultEntries {
            rotation: vec![],
            mine: vec![mine(1, server)],
            recovery: vec![],
        };
        let plan =
            plan_entry_unlock(Some(&local), &entries, &VaultSecret::Passphrase("pw")).unwrap();

        assert!(!plan.reconciled);
        assert_eq!(plan.install[0].1.expose(), server_dek.expose());
        apply_entry_unlock(
            &s,
            Some(&local),
            &entries,
            &plan,
            &VaultSecret::Passphrase("pw"),
        )
        .unwrap();
        assert_eq!(s.vault_record().unwrap().unwrap(), before);
        assert_eq!(meta(&s, "vault_conflict"), Some(1));
        assert_eq!(meta(&s, "vault_migrated"), None);
    }

    #[test]
    fn a_workspace_vault_that_does_not_open_never_locks_this_device_out() {
        // Device A's own vault has a DIFFERENT passphrase from the one the
        // workspace vault was seeded with. Typing A's passphrase must still
        // open A's vault rather than failing on the workspace entries.
        let s = store();
        let (local, _rk, local_dek) = crate::vault::setup("mine").unwrap();
        conflicted(&s, &local);
        let entries = VaultEntries {
            rotation: vec![],
            mine: vec![mine(1, crate::vault::setup("theirs").unwrap().0)],
            recovery: vec![],
        };

        let plan =
            plan_entry_unlock(Some(&local), &entries, &VaultSecret::Passphrase("mine")).unwrap();

        assert!(!plan.reconciled);
        assert_eq!(plan.install.len(), 1);
        assert_eq!(plan.install[0].1.expose(), local_dek.expose());
        apply_entry_unlock(
            &s,
            Some(&local),
            &entries,
            &plan,
            &VaultSecret::Passphrase("mine"),
        )
        .unwrap();
        assert_eq!(meta(&s, "vault_conflict"), Some(1));
        assert_eq!(meta(&s, "vault_migrated"), None);

        // A secret that opens neither is still a plain refusal.
        assert_eq!(
            err_of(plan_entry_unlock(
                Some(&local),
                &entries,
                &VaultSecret::Passphrase("neither")
            )),
            "wrong passphrase"
        );
    }

    /// R6: a member invited at generation 2 sees a workspace ring with no
    /// generation 1 in it. That is UNKNOWN, not a mismatch with their local
    /// record — installing the local generation 1 instead would leave them
    /// unable to read the workspace's notes at all.
    #[test]
    fn a_workspace_ring_without_generation_one_is_preferred_over_the_local_record() {
        let s = store();
        // Same passphrase on both sides, two different vaults.
        let (local, _rk, local_dek) = crate::vault::setup("pw").unwrap();
        s.set_vault_record(&local.to_json()).unwrap();
        let ws_dek = Dek::random();
        let entries = VaultEntries {
            mine: vec![MyEntry::try_from(my_entry_for(&ws_dek, 2, "pw")).unwrap()],
            recovery: vec![],
            rotation: vec![],
        };

        let plan =
            plan_entry_unlock(Some(&local), &entries, &VaultSecret::Passphrase("pw")).unwrap();

        assert_eq!(
            plan.install.iter().map(|(g, _)| *g).collect::<Vec<_>>(),
            vec![2],
            "the workspace ring, never mixed with a local generation 1"
        );
        assert_eq!(plan.install[0].1.expose(), ws_dek.expose());
        assert_ne!(plan.install[0].1.expose(), local_dek.expose());
        assert!(
            !plan.reconciled,
            "nothing proved the two are one vault — nothing may be written"
        );

        let before = s.vault_record().unwrap().unwrap();
        apply_entry_unlock(
            &s,
            Some(&local),
            &entries,
            &plan,
            &VaultSecret::Passphrase("pw"),
        )
        .unwrap();
        assert_eq!(s.vault_record().unwrap().unwrap(), before, "untouched");
    }

    /// R7: a passphrase change on another device rewraps the WORKSPACE
    /// entries. This device's own record is out of that loop, so the old
    /// passphrase would keep opening the vault locally — the one thing the
    /// change was supposed to stop. A reconciled passphrase unlock rewraps it.
    #[test]
    fn a_reconciled_passphrase_unlock_revokes_the_old_passphrase_locally() {
        let s = store();
        let (local, rk, dek) = crate::vault::setup("current").unwrap();
        s.set_vault_record(&local.to_json()).unwrap();
        // Another device already moved the workspace to "next".
        let entries = VaultEntries {
            mine: vec![mine(
                1,
                crate::vault::rewrap_passphrase(&local, &dek, "next"),
            )],
            recovery: vec![recovery_of(1, &local)],
            rotation: vec![],
        };

        let plan =
            plan_entry_unlock(Some(&local), &entries, &VaultSecret::Passphrase("next")).unwrap();
        assert!(plan.reconciled, "the same DEK — provably one vault");
        apply_entry_unlock(
            &s,
            Some(&local),
            &entries,
            &plan,
            &VaultSecret::Passphrase("next"),
        )
        .unwrap();

        let stored = load_vault_record(&s).unwrap();
        assert!(
            vault_unlock_passphrase(&stored, "current").is_err(),
            "the old passphrase no longer opens this device's record"
        );
        assert_eq!(
            vault_unlock_passphrase(&stored, "next").unwrap().expose(),
            dek.expose()
        );
        // The recovery key is untouched by a passphrase change.
        assert_eq!(
            vault_unlock_recovery(&stored, rk.as_str())
                .unwrap()
                .expose(),
            dek.expose()
        );
    }

    /// ...but a RECOVERY unlock only heals the check. A recovery key is not a
    /// passphrase and must never quietly become one.
    #[test]
    fn a_reconciled_recovery_unlock_leaves_the_passphrase_alone() {
        let s = store();
        let (rec, rk, dek) = crate::vault::setup("pw").unwrap();
        let mut checkless = rec.clone();
        checkless.dek_check = None;
        s.set_vault_record(&checkless.to_json()).unwrap();
        let entries = VaultEntries {
            mine: vec![mine(1, rec.clone())],
            recovery: vec![recovery_of(1, &rec)],
            rotation: vec![],
        };

        let plan = plan_entry_unlock(
            Some(&checkless),
            &entries,
            &VaultSecret::Recovery(rk.as_str()),
        )
        .unwrap();
        apply_entry_unlock(
            &s,
            Some(&checkless),
            &entries,
            &plan,
            &VaultSecret::Recovery(rk.as_str()),
        )
        .unwrap();

        let stored = load_vault_record(&s).unwrap();
        assert!(stored.dek_check.is_some(), "the check was healed");
        assert_eq!(
            vault_unlock_passphrase(&stored, "pw").unwrap().expose(),
            dek.expose(),
            "the passphrase still opens it — nothing was rewrapped"
        );
    }

    #[test]
    fn an_entry_whose_check_belongs_to_another_dek_is_rejected() {
        // The wrap opens, but the check proves the DEK is not this vault's —
        // exactly the foreign-key case `verify_dek` exists for.
        let (mut rec, _rk, _dek) = crate::vault::setup("pw").unwrap();
        rec.dek_check = Some(crate::vault::make_dek_check(&Dek::random()));
        let entries = VaultEntries {
            rotation: vec![],
            mine: vec![mine(1, rec)],
            recovery: vec![],
        };
        assert_eq!(
            err_of(unlock_entries_with_passphrase(&entries, "pw")),
            "wrong passphrase"
        );
    }

    #[test]
    fn decode_opt_b64_maps_an_empty_string_to_none() {
        assert_eq!(decode_opt_b64("").unwrap(), None);
        assert_eq!(
            decode_opt_b64(&STANDARD.encode(b"hi")).unwrap(),
            Some(b"hi".to_vec())
        );
        assert!(decode_opt_b64("not base64!!").is_err());
    }

    #[test]
    fn the_servers_literal_vault_keys_body_parses_and_unlocks() {
        // Every other entries test round-trips our own `to_json`, which
        // cannot catch a naming/encoding disagreement with the server. This
        // one is hand-written the way the server echoes the body back:
        // camelCase entry fields, and `kdfParams` verbatim as the client
        // wrote it (snake_case cost fields, salt as a JSON number array).
        let (rec, rk, dek) = crate::vault::setup("pw").unwrap();
        let check = STANDARD.encode(rec.dek_check.as_deref().unwrap());
        let salt = rec
            .kdf_params
            .salt
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let body = format!(
            concat!(
                r#"{{"mine":[{{"generation":1,"#,
                r#""kdfParams":{{"salt":[{salt}],"m_cost":{m},"t_cost":{t},"p_cost":{p}}},"#,
                r#""dekWrapped":"{wrapped}","dekCheck":"{check}"}}],"#,
                r#""recovery":[{{"generation":1,"recoverySalt":"{rsalt}","#,
                r#""dekWrappedRecovery":"{rwrapped}","dekCheck":"{check}"}}],"#,
                // Since the rotation-codes step the server always sends this
                // third list — empty while nothing is waiting to be redeemed.
                r#""rotation":[]}}"#
            ),
            salt = salt,
            m = rec.kdf_params.m_cost,
            t = rec.kdf_params.t_cost,
            p = rec.kdf_params.p_cost,
            wrapped = STANDARD.encode(&rec.dek_wrapped_pass),
            check = check,
            rsalt = STANDARD.encode(rec.recovery_salt),
            rwrapped = STANDARD.encode(&rec.dek_wrapped_recovery),
        );

        let entries = VaultEntries::from_json(&body).unwrap();

        assert_eq!(entries.mine.len(), 1);
        assert_eq!(entries.recovery.len(), 1);
        assert_eq!(
            unlock_entries_with_passphrase(&entries, "pw").unwrap()[0]
                .1
                .expose(),
            dek.expose(),
            "the server's own body must open for real"
        );
        assert_eq!(
            unlock_entries_with_recovery(&entries, rk.as_str()).unwrap()[0]
                .1
                .expose(),
            dek.expose()
        );
        // ...and what we send back has exactly that shape.
        let ours: serde_json::Value = serde_json::from_str(&entries.to_json()).unwrap();
        let theirs: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(ours, theirs);
    }

    #[test]
    fn rewrap_for_server_produces_one_upload_per_generation_and_keeps_the_dek() {
        let (local, rk, dek) = crate::vault::setup("old").unwrap();
        let entries = VaultEntries {
            rotation: vec![],
            mine: vec![mine(
                1,
                crate::vault::rewrap_passphrase(&local, &dek, "old"),
            )],
            recovery: vec![recovery_of(1, &local)],
        };

        let out = rewrap_for_server(Some(&local), &entries, "old", "new").unwrap();

        assert_eq!(out.uploads.len(), 1);
        assert_eq!(out.uploads[0].generation, 1);
        assert_eq!(out.deks.len(), 1);
        assert_eq!(out.deks[0].1.expose(), dek.expose(), "same DEK, new wrap");

        // The rewrapped cache opens with the NEW passphrase only.
        assert!(unlock_entries_with_passphrase(&out.entries, "old").is_err());
        assert_eq!(
            unlock_entries_with_passphrase(&out.entries, "new").unwrap()[0]
                .1
                .expose(),
            dek.expose()
        );
        // The recovery wrap is carried through untouched.
        assert_eq!(
            unlock_entries_with_recovery(&out.entries, rk.as_str()).unwrap()[0]
                .1
                .expose(),
            dek.expose()
        );
        // And so is the local record, rewrapped under the new passphrase.
        let new_local = out.record.unwrap();
        assert!(vault_unlock_passphrase(&new_local, "old").is_err());
        assert_eq!(
            vault_unlock_passphrase(&new_local, "new").unwrap().expose(),
            dek.expose()
        );
    }

    /// R3: a PUT loop that fails on generation 3 of 3 has to put generations
    /// 1 and 2 back under the OLD passphrase, or no single passphrase opens
    /// the whole ring any more.
    #[test]
    fn a_partial_rewrap_upload_reverts_exactly_the_generations_that_landed() {
        let (rec, _rk, dek) = crate::vault::setup("old").unwrap();
        let entries = VaultEntries {
            mine: (1..=3)
                .map(|g| mine(g, crate::vault::rewrap_passphrase(&rec, &dek, "old")))
                .collect(),
            recovery: vec![],
            rotation: vec![],
        };

        // Nothing landed yet: nothing to undo.
        assert!(rewrap_revert_uploads(&entries, &[]).is_empty());

        // Generations 1 and 2 landed before the failure.
        let revert = rewrap_revert_uploads(&entries, &[2, 1]);
        assert_eq!(
            revert.iter().map(|e| e.generation).collect::<Vec<_>>(),
            vec![1, 2],
            "ascending, whatever order they were uploaded in"
        );
        // And what goes back is the OLD wrap — it still opens with "old".
        let reverted = VaultEntries {
            mine: revert
                .into_iter()
                .map(MyEntry::try_from)
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            recovery: vec![],
            rotation: vec![],
        };
        assert_eq!(
            unlock_entries_with_passphrase(&reverted, "old")
                .unwrap()
                .len(),
            2
        );

        // A generation the cache does not describe is skipped, not guessed.
        assert!(rewrap_revert_uploads(&entries, &[9]).is_empty());
    }

    #[test]
    fn rewrap_for_server_refuses_a_wrong_current_passphrase() {
        let (local, _rk, dek) = crate::vault::setup("old").unwrap();
        let entries = VaultEntries {
            rotation: vec![],
            mine: vec![mine(
                1,
                crate::vault::rewrap_passphrase(&local, &dek, "old"),
            )],
            recovery: vec![],
        };
        assert_eq!(
            err_of(rewrap_for_server(Some(&local), &entries, "nope", "new")),
            "wrong passphrase"
        );
    }

    #[test]
    fn migration_plan_uploads_only_an_unmigrated_local_record_on_a_vaultless_workspace() {
        let s = store();
        // No local record yet -> nothing to migrate.
        assert!(matches!(
            vault_migration_plan(&s).unwrap(),
            VaultMigration::None
        ));

        vault_setup(&s, "pw").unwrap();
        crate::migrate::set_meta_i64(&s.conn, "vault_generation", 0).unwrap();
        assert!(matches!(
            vault_migration_plan(&s).unwrap(),
            VaultMigration::Upload(_)
        ));

        // The workspace already has a vault this record did not create.
        crate::migrate::set_meta_i64(&s.conn, "vault_generation", 1).unwrap();
        assert!(matches!(
            vault_migration_plan(&s).unwrap(),
            VaultMigration::Conflict
        ));

        // Once migrated (or against a legacy server) the hook stays quiet.
        crate::migrate::set_meta_i64(&s.conn, "vault_migrated", 1).unwrap();
        assert!(matches!(
            vault_migration_plan(&s).unwrap(),
            VaultMigration::None
        ));
        crate::migrate::delete_meta(&s.conn, "vault_migrated").unwrap();
        crate::migrate::set_meta_i64(&s.conn, "vault_server_legacy", 1).unwrap();
        assert!(matches!(
            vault_migration_plan(&s).unwrap(),
            VaultMigration::None
        ));
    }

    #[test]
    fn migration_plan_skips_a_record_that_has_no_dek_check_yet() {
        let s = store();
        let (mut rec, _rk, _dek) = crate::vault::setup("pw").unwrap();
        rec.dek_check = None;
        s.set_vault_record(&rec.to_json()).unwrap();
        crate::migrate::set_meta_i64(&s.conn, "vault_generation", 0).unwrap();
        assert!(
            matches!(vault_migration_plan(&s).unwrap(), VaultMigration::None),
            "skipped silently until an unlock self-heals the record"
        );
    }

    #[test]
    fn unlock_inputs_read_both_halves_in_one_lock_scope() {
        let s = store();
        let empty = load_vault_unlock_inputs(&s).unwrap();
        assert!(empty.record.is_none() && empty.entries.is_none());

        vault_setup(&s, "pw").unwrap();
        let (rec, _rk, _dek) = crate::vault::setup("pw").unwrap();
        s.set_vault_entries(
            &VaultEntries {
                rotation: vec![],
                mine: vec![mine(1, rec)],
                recovery: vec![],
            }
            .to_json(),
        )
        .unwrap();

        let both = load_vault_unlock_inputs(&s).unwrap();
        assert!(both.record.is_some());
        assert_eq!(both.entries.unwrap().mine.len(), 1);
    }

    #[test]
    fn a_corrupt_entry_cache_is_ignored_rather_than_failing_the_unlock() {
        let s = store();
        vault_setup(&s, "pw").unwrap();
        s.set_vault_entries("not json at all").unwrap();
        assert!(
            cached_vault_entries(&s).unwrap().is_none(),
            "unparsable cache is reported as absent, never a panic"
        );
        let inputs = load_vault_unlock_inputs(&s).unwrap();
        assert!(
            inputs.entries.is_none(),
            "unparsable cache falls back to the local record"
        );
        assert!(inputs.record.is_some());
    }
}

/// Resolving a vault conflict: a device whose local-only vault met an
/// existing workspace vault moves every note sealed under its own key out of
/// that vault and becomes a normal workspace device.
#[cfg(test)]
mod conflict_resolution_tests {
    use super::test_support::*;
    use super::*;

    /// A workspace with generations 1 and 2 (passphrase "ws-pw"), a local
    /// record with its own DEK (passphrase "local-pw"), three notes sealed
    /// locally, one sealed under workspace generation 2.
    fn conflicted_store() -> (Store, VaultEntries, Vec<(u32, Dek)>, Dek) {
        let mut s = Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        s.sync_enabled = true;
        let (rec1, _rk, d1) = crate::vault::setup("ws-pw").unwrap();
        let d2 = Dek::random();
        let rec2 =
            crate::vault::rewrap_passphrase(&crate::vault::setup("ws-pw").unwrap().0, &d2, "ws-pw");
        let entries = VaultEntries {
            mine: vec![
                MyEntry {
                    generation: 1,
                    record: rec1,
                },
                MyEntry {
                    generation: 2,
                    record: rec2,
                },
            ],
            recovery: vec![],
            rotation: vec![],
        };
        let (local_rec, _lrk, local_dek) = crate::vault::setup("local-pw").unwrap();
        s.set_vault_entries(&entries.to_json()).unwrap();
        s.set_vault_record(&local_rec.to_json()).unwrap();
        crate::migrate::set_meta_i64(&s.conn, "vault_conflict", 1).unwrap();
        for id in ["a", "b", "c"] {
            s.save_note(&Note {
                id: id.into(),
                content: format!("<p>{id}</p>"),
                updated_at: 1,
                ..Default::default()
            })
            .unwrap();
            s.set_note_protected(id, true).unwrap();
            encrypt_note_in_place(&s, id, &local_dek, 1).unwrap();
        }
        s.save_note(&Note {
            id: "w".into(),
            content: "<p>w</p>".into(),
            updated_at: 1,
            ..Default::default()
        })
        .unwrap();
        s.set_note_protected("w", true).unwrap();
        encrypt_note_in_place(&s, "w", &d2, 2).unwrap();
        let dirty: Vec<_> = s
            .load_dirty_notes()
            .unwrap()
            .iter()
            .map(|n| (n.id.clone(), n.updated_at))
            .collect();
        s.clear_note_dirty(&dirty).unwrap();
        (s, entries, vec![(1, d1), (2, d2)], local_dek)
    }

    #[test]
    fn merging_reseals_the_local_notes_under_the_newest_workspace_key() {
        let (s, entries, ring, local_dek) = conflicted_store();
        let inputs = load_conflict_inputs(&s).unwrap();
        let (opened, dek) =
            open_conflict_sides(&inputs, "ws-pw", &VaultSecret::Passphrase("local-pw")).unwrap();
        assert_eq!(
            opened.iter().map(|(g, _)| *g).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(dek.expose(), local_dek.expose());

        let out = resolve_conflict(&s, &ring, &local_dek, ConflictMode::Merge).unwrap();
        assert_eq!(
            out,
            ConflictOutcome {
                changed: 3,
                skipped: 0
            }
        );
        let mut vault = VaultState::default();
        for (g, d) in &ring {
            vault.unlock(*g, d.clone());
        }
        for id in ["a", "b", "c"] {
            assert_eq!(s.note_key_gen(id).unwrap(), Some(2));
            assert_eq!(
                open_note_content(&s, &vault, id).unwrap(),
                format!("<p>{id}</p>")
            );
        }
        assert_eq!(s.note_key_gen("w").unwrap(), Some(2));
        assert_eq!(
            s.load_dirty_notes().unwrap().len(),
            3,
            "only the re-sealed notes are pushed"
        );

        finish_conflict_resolution(&s, &entries).unwrap();
        assert!(crate::migrate::get_meta_i64_opt(&s.conn, "vault_conflict")
            .unwrap()
            .is_none());
        assert_eq!(
            crate::migrate::get_meta_i64_opt(&s.conn, "vault_migrated").unwrap(),
            Some(1)
        );
        let rec = load_vault_record(&s).unwrap();
        assert_eq!(
            crate::vault::unlock_passphrase(&rec, "ws-pw")
                .unwrap()
                .expose(),
            ring[0].1.expose(),
            "the local record now mirrors generation 1"
        );
    }

    #[test]
    fn unprotecting_stores_the_local_notes_as_plaintext_and_keeps_folder_locks() {
        let (s, _entries, ring, local_dek) = conflicted_store();
        crate::folders::create_folder(&s.conn, "f", "F", None).unwrap();
        s.set_folder_locked("f", true).unwrap();
        let out = resolve_conflict(&s, &ring, &local_dek, ConflictMode::Unprotect).unwrap();
        assert_eq!(
            out,
            ConflictOutcome {
                changed: 3,
                skipped: 0
            }
        );
        for id in ["a", "b", "c"] {
            assert!(!s.note_protected(id).unwrap());
            assert_eq!(s.note_key_gen(id).unwrap(), None);
            assert_eq!(
                s.load_note_content(id).unwrap().unwrap(),
                format!("<p>{id}</p>")
            );
        }
        assert!(
            s.note_protected("w").unwrap(),
            "workspace-sealed notes are untouched"
        );
        assert!(
            s.folder_locked("f").unwrap(),
            "folder locks stay as they are"
        );
        assert_eq!(s.load_dirty_notes().unwrap().len(), 3);
    }

    /// A protected note in the TRASH is sealed under the local DEK just like a
    /// live one, and the resolution replaces the only wrap of that DEK — so
    /// leaving it behind would hand the user ciphertext nobody can open the
    /// moment they restore it.
    #[test]
    fn a_trashed_note_moves_out_of_the_local_vault_with_the_rest() {
        let (s, _entries, ring, local_dek) = conflicted_store();
        s.trash_note("b", 500).unwrap();
        let out = resolve_conflict(&s, &ring, &local_dek, ConflictMode::Merge).unwrap();
        assert_eq!(
            out,
            ConflictOutcome {
                changed: 3,
                skipped: 0
            },
            "the trashed note is counted like any other"
        );
        let mut vault = VaultState::default();
        for (g, d) in &ring {
            vault.unlock(*g, d.clone());
        }
        assert_eq!(s.note_key_gen("b").unwrap(), Some(2));
        s.restore_note("b").unwrap();
        assert_eq!(open_note_content(&s, &vault, "b").unwrap(), "<p>b</p>");

        // Same in the other mode: unprotected, not left sealed.
        let (s, _entries, ring, local_dek) = conflicted_store();
        s.trash_note("b", 500).unwrap();
        let out = resolve_conflict(&s, &ring, &local_dek, ConflictMode::Unprotect).unwrap();
        assert_eq!(
            out,
            ConflictOutcome {
                changed: 3,
                skipped: 0
            }
        );
        assert!(!s.note_protected("b").unwrap());
        assert_eq!(s.load_note_content("b").unwrap().unwrap(), "<p>b</p>");
    }

    /// Unprotecting a note under a locked folder would leave `protected = 0`
    /// below a locked ancestor — the state `set_note_protected(false)`
    /// refuses. It is merged instead, and the lock itself is never touched.
    #[test]
    fn unprotect_merges_a_note_inside_a_locked_folder_and_frees_only_the_rest() {
        let (s, _entries, ring, local_dek) = conflicted_store();
        crate::folders::create_folder(&s.conn, "f", "F", None).unwrap();
        s.set_folder_locked("f", true).unwrap();
        s.conn
            .execute("UPDATE notes SET folder_id = 'f' WHERE id = 'a'", [])
            .unwrap();

        let out = resolve_conflict(&s, &ring, &local_dek, ConflictMode::Unprotect).unwrap();
        assert_eq!(
            out,
            ConflictOutcome {
                changed: 3,
                skipped: 0
            },
            "the merged note counts as changed too"
        );

        let mut vault = VaultState::default();
        for (g, d) in &ring {
            vault.unlock(*g, d.clone());
        }
        assert!(
            s.note_protected("a").unwrap(),
            "a locked folder overrides unprotect"
        );
        assert_eq!(s.note_key_gen("a").unwrap(), Some(2));
        assert_eq!(open_note_content(&s, &vault, "a").unwrap(), "<p>a</p>");
        for id in ["b", "c"] {
            assert!(!s.note_protected(id).unwrap(), "the siblings outside f");
            assert_eq!(s.note_key_gen(id).unwrap(), None);
            assert_eq!(
                s.load_note_content(id).unwrap().unwrap(),
                format!("<p>{id}</p>")
            );
        }
        assert!(s.folder_locked("f").unwrap(), "the lock is never touched");
        assert_eq!(s.load_dirty_notes().unwrap().len(), 3);
    }

    #[test]
    fn a_note_neither_key_opens_is_skipped_and_counted() {
        let (s, _entries, ring, local_dek) = conflicted_store();
        s.save_note(&Note {
            id: "x".into(),
            content: "<p>x</p>".into(),
            updated_at: 1,
            ..Default::default()
        })
        .unwrap();
        s.set_note_protected("x", true).unwrap();
        encrypt_note_in_place(&s, "x", &Dek::random(), 1).unwrap();
        let out = resolve_conflict(&s, &ring, &local_dek, ConflictMode::Merge).unwrap();
        assert_eq!(
            out,
            ConflictOutcome {
                changed: 3,
                skipped: 1
            }
        );
        assert!(s.note_protected("x").unwrap());
    }

    #[test]
    fn the_same_key_on_both_sides_changes_nothing() {
        let (s, _entries, ring, _local_dek) = conflicted_store();
        let out = resolve_conflict(&s, &ring, &ring[0].1, ConflictMode::Merge).unwrap();
        assert_eq!(
            out,
            ConflictOutcome {
                changed: 0,
                skipped: 0
            }
        );
        assert!(s.load_dirty_notes().unwrap().is_empty());
    }

    /// The merge is a SEAL, so it obeys the same generation guard every other
    /// seal does: a device whose cached entries stop at generation 2 while the
    /// workspace has already rotated to 3 must not move its notes onto the
    /// key that rotation retired. Refused before the first note.
    #[test]
    fn merging_with_a_ring_behind_the_workspace_is_refused_and_writes_nothing() {
        let (s, _entries, ring, local_dek) = conflicted_store();
        crate::migrate::set_meta_i64(&s.conn, "vault_generation", 3).unwrap();

        let err = err_of(resolve_conflict(&s, &ring, &local_dek, ConflictMode::Merge));
        assert!(
            err.contains("key generation outdated"),
            "unexpected error: {err}"
        );

        // Not one note moved, and nothing queued for the server.
        let mut vault = VaultState::default();
        vault.unlock(1, local_dek.clone());
        for id in ["a", "b", "c"] {
            assert!(s.note_protected(id).unwrap());
            assert_eq!(s.note_key_gen(id).unwrap(), Some(1));
            assert_eq!(
                open_note_content(&s, &vault, id).unwrap(),
                format!("<p>{id}</p>")
            );
        }
        assert!(s.load_dirty_notes().unwrap().is_empty());

        // Unprotecting is refused too — a note under a locked folder takes the
        // merge path, so the same retired key would be in play.
        assert!(err_of(resolve_conflict(
            &s,
            &ring,
            &local_dek,
            ConflictMode::Unprotect
        ))
        .contains("key generation outdated"));
    }

    /// A cache that never held a generation-1 entry for this caller (they
    /// joined after a rotation, so their own wrap starts at 2) has nothing to
    /// mirror into a local record. The placeholder keeps the row empty rather
    /// than inventing one, and `vault_exists` stays true off the cache alone.
    #[test]
    fn finishing_without_a_generation_one_entry_leaves_an_empty_placeholder() {
        let (s, mut entries, _ring, _local_dek) = conflicted_store();
        entries.mine.retain(|e| e.generation != 1);
        s.set_vault_entries(&entries.to_json()).unwrap();

        finish_conflict_resolution(&s, &entries).unwrap();

        assert_eq!(s.vault_record().unwrap(), None, "no record is invented");
        assert!(
            vault_exists(&s).unwrap(),
            "the cached entries alone keep the vault present"
        );
        assert!(crate::migrate::get_meta_i64_opt(&s.conn, "vault_conflict")
            .unwrap()
            .is_none());
        assert_eq!(
            crate::migrate::get_meta_i64_opt(&s.conn, "vault_migrated").unwrap(),
            Some(1)
        );
    }

    #[test]
    fn wrong_secrets_are_named_and_write_nothing() {
        let (s, _entries, _ring, _local_dek) = conflicted_store();
        let inputs = load_conflict_inputs(&s).unwrap();
        assert_eq!(
            err_of(open_conflict_sides(
                &inputs,
                "nope",
                &VaultSecret::Passphrase("local-pw")
            )),
            "wrong passphrase"
        );
        assert_eq!(
            err_of(open_conflict_sides(
                &inputs,
                "ws-pw",
                &VaultSecret::Passphrase("nope")
            )),
            "vault: local record does not open"
        );
        crate::migrate::delete_meta(&s.conn, "vault_conflict").unwrap();
        assert_eq!(
            err_of(load_conflict_inputs(&s)),
            "vault: no conflict to resolve"
        );
    }
}

/// Sharing a workspace vault through a one-time invite code, and what the
/// invitee's device settles locally once the server accepted their own wrap.
#[cfg(test)]
mod vault_invite_tests {
    use super::test_support::{err_of, store};
    use super::*;

    #[test]
    fn invite_wrap_opens_with_the_code_and_only_the_code() {
        let dek = Dek::random();
        let (code, wrap) = make_invite_wrap(&dek, 2);
        assert_eq!(wrap.generation, 2);
        assert_eq!(
            open_invite_wrap(&wrap, &code).unwrap().expose(),
            dek.expose()
        );
        assert_eq!(
            open_invite_wrap(&wrap, &code.to_lowercase().replace('-', " "))
                .unwrap()
                .expose(),
            dek.expose(),
            "formatting-tolerant"
        );
        assert_eq!(
            err_of(open_invite_wrap(&wrap, "AAAA-AAAA-AAAA-AAAA-AAAA-AAAA-AA")),
            "invalid invite code"
        );
        let entry = my_entry_for(&dek, 2, "member-pw");
        let rec =
            VaultEntries::from_json(&serde_json::json!({"mine":[entry],"recovery":[]}).to_string())
                .unwrap();
        assert_eq!(
            unlock_entries_with_passphrase(&rec, "member-pw").unwrap()[0].0,
            2
        );
    }

    #[test]
    fn a_corrupt_invite_wrap_is_rejected_like_a_wrong_code() {
        let dek = Dek::random();
        let (code, wrap) = make_invite_wrap(&dek, 1);

        let bad_wrap = InviteWrap {
            dek_wrapped: "not base64!!".to_string(),
            ..wrap.clone()
        };
        assert_eq!(
            err_of(open_invite_wrap(&bad_wrap, &code)),
            "invalid invite code"
        );
        let bad_check = InviteWrap {
            dek_check: "not base64!!".to_string(),
            ..wrap.clone()
        };
        assert_eq!(
            err_of(open_invite_wrap(&bad_check, &code)),
            "invalid invite code"
        );
        // A check that belongs to a DIFFERENT vault: the unwrap succeeds, the
        // proof does not.
        let foreign = InviteWrap {
            dek_check: STANDARD.encode(crate::vault::make_dek_check(&Dek::random())),
            ..wrap
        };
        assert_eq!(
            err_of(open_invite_wrap(&foreign, &code)),
            "invalid invite code"
        );
    }

    #[test]
    fn the_invite_wrap_travels_as_the_server_names_it_in_both_directions() {
        let dek = Dek::random();
        let (_code, wrap) = make_invite_wrap(&dek, 3);
        let sent: serde_json::Value = serde_json::from_str(&serde_json::to_string(&wrap).unwrap())
            .expect("InviteWrap serializes");
        assert_eq!(sent["generation"], 3);
        assert!(sent["dekWrapped"].is_string() && sent["dekCheck"].is_string());

        // The fetch endpoint answers with `dekWrappedInvite` instead.
        let fetched: InviteWrap = serde_json::from_value(serde_json::json!({
            "generation": 3,
            "kdfParams": sent["kdfParams"],
            "dekWrappedInvite": sent["dekWrapped"],
            "dekCheck": sent["dekCheck"],
        }))
        .unwrap();
        assert_eq!(fetched.dek_wrapped, wrap.dek_wrapped);
    }

    #[test]
    fn an_invitation_reference_is_an_id_a_link_or_a_bare_token() {
        assert_eq!(parse_invitation_ref(" 42 ").unwrap(), InvitationRef::Id(42));
        assert_eq!(
            parse_invitation_ref("https://notes.example.com/invite/abc123").unwrap(),
            InvitationRef::Token("abc123".into())
        );
        assert_eq!(
            parse_invitation_ref("https://notes.example.com/invite/abc123?ref=mail#top").unwrap(),
            InvitationRef::Token("abc123".into())
        );
        assert_eq!(
            parse_invitation_ref("/invite/abc123/").unwrap(),
            InvitationRef::Token("abc123".into())
        );
        assert_eq!(
            parse_invitation_ref("abc123").unwrap(),
            InvitationRef::Token("abc123".into())
        );
        assert_eq!(
            parse_invitation_ref("   ").unwrap_err(),
            "invitation: nothing entered"
        );
        assert_eq!(
            parse_invitation_ref("https://notes.example.com/invite/").unwrap_err(),
            "invitation: no token in that link"
        );

        // The token is interpolated into a request path, so anything that
        // could reshape that path is refused rather than sent.
        for odd in [
            "../../admin",
            "abc 123",
            "abc/def",
            "abc?x=1",
            "abc#frag",
            "https://notes.example.com/invite/../../admin",
        ] {
            assert_eq!(
                parse_invitation_ref(odd).unwrap_err(),
                "invitation: that does not look like an invitation link",
                "{odd} must not reach the URL"
            );
        }
        // What a real token looks like still goes through untouched.
        assert_eq!(
            parse_invitation_ref("aB3_x-Y9").unwrap(),
            InvitationRef::Token("aB3_x-Y9".into())
        );
    }

    #[test]
    fn accepting_replaces_the_entry_for_that_generation_and_keeps_the_rest() {
        let dek1 = Dek::random();
        let dek2 = Dek::random();
        let cached = VaultEntries::from_json(
            &serde_json::json!({
                "mine": [my_entry_for(&dek1, 1, "old-pw")],
                "recovery": [],
            })
            .to_string(),
        )
        .unwrap();

        let accepted = accept_invite_entry(
            Some(&cached),
            None,
            my_entry_for(&dek2, 2, "member-pw"),
            &dek2,
            "member-pw",
        )
        .unwrap();
        assert_eq!(
            accepted
                .entries
                .mine
                .iter()
                .map(|e| e.generation)
                .collect::<Vec<_>>(),
            vec![1, 2],
            "a new generation is added, not replaced"
        );

        // Accepting again for generation 1 replaces that entry in place.
        let again = accept_invite_entry(
            Some(&accepted.entries),
            None,
            my_entry_for(&dek1, 1, "new-pw"),
            &dek1,
            "new-pw",
        )
        .unwrap();
        assert_eq!(again.entries.mine.len(), 2);
        let opened = unlock_entries_with_passphrase(&again.entries, "new-pw").unwrap();
        assert_eq!(opened.len(), 1);
        assert_eq!(opened[0].0, 1);
        assert_eq!(opened[0].1.expose(), dek1.expose());
        assert!(
            unlock_entries_with_passphrase(&again.entries, "old-pw").is_err(),
            "the superseded wrap is gone"
        );
    }

    #[test]
    fn a_device_without_a_record_mirrors_generation_one_and_one_with_a_record_does_not() {
        let dek = Dek::random();

        let fresh =
            accept_invite_entry(None, None, my_entry_for(&dek, 1, "pw"), &dek, "pw").unwrap();
        let mirrored = fresh.record.expect("a new device gets a local record");
        assert_eq!(
            vault_unlock_passphrase(&mirrored, "pw").unwrap().expose(),
            dek.expose()
        );

        // Generation 2 only: nothing to mirror (the local record IS generation 1).
        assert!(
            accept_invite_entry(None, None, my_entry_for(&dek, 2, "pw"), &dek, "pw")
                .unwrap()
                .record
                .is_none()
        );

        // A device that already has a record keeps it untouched.
        let (local, _rk, _d) = crate::vault::setup("local-pw").unwrap();
        assert!(
            accept_invite_entry(None, Some(&local), my_entry_for(&dek, 1, "pw"), &dek, "pw")
                .unwrap()
                .record
                .is_none()
        );
    }

    #[test]
    fn accepting_flags_the_conflict_unless_the_two_vaults_are_provably_one() {
        // (a) No local record: the mirrored one IS the workspace's key.
        let dek = Dek::random();
        assert!(
            !accept_invite_entry(None, None, my_entry_for(&dek, 1, "pw"), &dek, "pw")
                .unwrap()
                .conflict
        );

        // (b) A local record whose DEK is the one the invite handed over —
        // `dek_check` settles it without needing the local passphrase.
        let (local, _rk, local_dek) = crate::vault::setup("local-pw").unwrap();
        assert!(
            !accept_invite_entry(
                None,
                Some(&local),
                my_entry_for(&local_dek, 1, "member-pw"),
                &local_dek,
                "member-pw"
            )
            .unwrap()
            .conflict
        );

        // (c) A local record holding a DIFFERENT vault: the accept is
        // conflicted, so the flag is SET — the re-seal sweep stands down and
        // the banner keeps saying the pre-join notes are under another key.
        assert!(
            accept_invite_entry(
                None,
                Some(&local),
                my_entry_for(&dek, 1, "member-pw"),
                &dek,
                "member-pw"
            )
            .unwrap()
            .conflict
        );

        // (d) A record predating `dek_check` falls back to opening it with the
        // new passphrase — which only works when it really is the same vault.
        let mut checkless = crate::vault::rewrap_passphrase(&local, &local_dek, "member-pw");
        checkless.dek_check = None;
        assert!(
            !accept_invite_entry(
                None,
                Some(&checkless),
                my_entry_for(&local_dek, 1, "member-pw"),
                &local_dek,
                "member-pw"
            )
            .unwrap()
            .conflict
        );
        assert!(
            accept_invite_entry(
                None,
                Some(&checkless),
                my_entry_for(&dek, 1, "member-pw"),
                &dek,
                "member-pw"
            )
            .unwrap()
            .conflict
        );
    }

    /// C1(b): the two store-side branches of an accept, end to end.
    #[test]
    fn applying_an_accept_settles_the_conflict_and_migrated_flags() {
        let dek = Dek::random();

        // Conflict-free: `vault_migrated` claimed, `vault_conflict` cleared.
        let s = store();
        crate::migrate::set_meta_i64(&s.conn, "vault_conflict", 1).unwrap();
        let clean =
            accept_invite_entry(None, None, my_entry_for(&dek, 1, "pw"), &dek, "pw").unwrap();
        apply_accepted_invite(&s, &clean).unwrap();
        assert!(crate::migrate::get_meta_i64_opt(&s.conn, "vault_conflict")
            .unwrap()
            .is_none());
        assert_eq!(
            crate::migrate::get_meta_i64_opt(&s.conn, "vault_migrated").unwrap(),
            Some(1)
        );
        assert!(s.vault_record().unwrap().is_some(), "generation 1 mirrored");

        // Conflicted: `vault_conflict` SET, `vault_migrated` deliberately not
        // claimed, so the sync hook keeps re-marking the conflict.
        let s2 = store();
        let (local, _rk, _d) = crate::vault::setup("local-pw").unwrap();
        s2.set_vault_record(&local.to_json()).unwrap();
        let clashing =
            accept_invite_entry(None, Some(&local), my_entry_for(&dek, 1, "pw"), &dek, "pw")
                .unwrap();
        apply_accepted_invite(&s2, &clashing).unwrap();
        assert_eq!(
            crate::migrate::get_meta_i64_opt(&s2.conn, "vault_conflict").unwrap(),
            Some(1)
        );
        assert!(crate::migrate::get_meta_i64_opt(&s2.conn, "vault_migrated")
            .unwrap()
            .is_none());
    }

    /// C1(b): redeeming a rotation code never clears a conflict — it proves
    /// nothing about this device's own record.
    #[test]
    fn redeeming_a_rotation_leaves_the_conflict_flag_alone() {
        let s = store();
        crate::migrate::set_meta_i64(&s.conn, "vault_conflict", 1).unwrap();
        let dek = Dek::random();
        apply_rotation_redeem(&s, my_entry_for(&dek, 2, "pw")).unwrap();
        assert_eq!(
            crate::migrate::get_meta_i64_opt(&s.conn, "vault_conflict").unwrap(),
            Some(1),
            "a redeem must not clear the conflict"
        );
        let cached = cached_vault_entries(&s).unwrap().unwrap();
        assert_eq!(
            cached.mine.iter().map(|e| e.generation).collect::<Vec<_>>(),
            vec![2]
        );
    }

    #[test]
    fn only_a_workspace_member_holding_a_recovery_wrap_counts_as_a_recovery_holder() {
        let dek = Dek::random();
        let mine_only = VaultEntries::from_json(
            &serde_json::json!({"mine": [my_entry_for(&dek, 1, "pw")], "recovery": []}).to_string(),
        )
        .unwrap();

        // A local context always is: its recovery key was minted here.
        assert!(vault_recovery_holder(None, None, false));
        assert!(vault_recovery_holder(Some(&mine_only), None, false));
        // On a workspace, an invited member holds wraps but no recovery key.
        assert!(!vault_recovery_holder(Some(&mine_only), None, true));
        assert!(!vault_recovery_holder(None, None, true));

        let (rec, _rk, _d) = crate::vault::setup("pw").unwrap();
        let with_recovery = VaultEntries {
            rotation: vec![],
            mine: mine_only.mine.clone(),
            recovery: vec![RecoveryEntry {
                generation: 1,
                recovery_salt: rec.recovery_salt,
                dek_wrapped_recovery: rec.dek_wrapped_recovery.clone(),
                dek_check: rec.dek_check.clone(),
            }],
        };
        assert!(vault_recovery_holder(Some(&with_recovery), None, true));
    }

    #[test]
    fn the_vault_creator_is_a_recovery_holder_before_the_first_pull_caches_anything() {
        // `vault_setup` on a server context writes a full local record long
        // before a pull can hand the entries back. Its owner was shown a
        // recovery key and it still works, so the controls must stay.
        let (created, _rk, _dek) = crate::vault::setup("pw").unwrap();
        assert!(vault_recovery_holder(None, Some(&created), true));

        // The invitee's MIRRORED record carries a zero salt and an empty
        // recovery wrap — nothing to offer.
        let dek = Dek::random();
        let mirrored = accept_invite_entry(None, None, my_entry_for(&dek, 1, "pw"), &dek, "pw")
            .unwrap()
            .record
            .expect("generation 1 mirrors a record");
        assert!(mirrored.dek_wrapped_recovery.is_empty());
        assert!(!vault_recovery_holder(None, Some(&mirrored), true));
        // ...and a local context is a holder either way.
        assert!(vault_recovery_holder(None, Some(&mirrored), false));
    }

    /// Round 2 / Important 1(b): the flag the UI uses to show protected notes
    /// read-only instead of letting the user type into a note whose save the
    /// backend would refuse. Mirrors `guard_seal_generation` exactly.
    #[test]
    fn seal_outdated_mirrors_the_seal_guard() {
        // Behind the workspace: outdated.
        assert!(seal_outdated(3, Some(1), true, false, true));
        assert!(seal_outdated(2, Some(1), true, false, true));
        // Caught up, or ahead of a workspace that has not rotated: not.
        assert!(!seal_outdated(1, Some(1), true, false, true));
        assert!(!seal_outdated(0, Some(1), true, false, true));
        assert!(!seal_outdated(2, Some(3), true, false, true));
        // A local context has no workspace generation to be behind.
        assert!(!seal_outdated(3, Some(1), false, false, false));
        // Conflicted AND sealing with this device's OWN vault: the two
        // numberings are unrelated, so no comparison is made.
        assert!(!seal_outdated(3, Some(1), true, true, false));
        // Conflicted but sealing with a WORKSPACE key: compared as usual —
        // that key is exactly the one the rotation was meant to retire.
        assert!(seal_outdated(3, Some(1), true, true, true));
        assert!(!seal_outdated(3, Some(3), true, true, true));
        // A locked ring is reported as "locked", never as "outdated key":
        // protected notes already show the locked placeholder there.
        assert!(!seal_outdated(3, None, true, false, false));
    }

    /// Round 3 / minor 1: generation NUMBERS cannot tell a workspace key from
    /// a private one — both call their first key generation 1. The cached
    /// entry's `dek_check` is what settles it.
    #[test]
    fn a_workspace_key_is_recognised_by_proof_not_by_its_number() {
        let workspace_dek = Dek::random();
        let own_dek = Dek::random();
        let entries = VaultEntries {
            mine: vec![MyEntry::try_from(my_entry_for(&workspace_dek, 1, "pw")).unwrap()],
            recovery: vec![],
            rotation: vec![],
        };

        assert!(ring_key_is_the_workspaces(
            Some(&entries),
            1,
            &workspace_dek
        ));
        // Same generation NUMBER, different key: this device's own vault.
        assert!(!ring_key_is_the_workspaces(Some(&entries), 1, &own_dek));
        // A generation the workspace never handed over.
        assert!(!ring_key_is_the_workspaces(
            Some(&entries),
            2,
            &workspace_dek
        ));
        // Nothing cached proves nothing.
        assert!(!ring_key_is_the_workspaces(None, 1, &workspace_dek));
    }

    /// The flags carry that answer for the ring's newest generation.
    #[test]
    fn the_status_flags_report_whether_the_ring_is_the_workspaces() {
        let s = store();
        let workspace_dek = Dek::random();
        s.set_vault_entries(
            &VaultEntries {
                mine: vec![MyEntry::try_from(my_entry_for(&workspace_dek, 2, "pw")).unwrap()],
                recovery: vec![],
                rotation: vec![],
            }
            .to_json(),
        )
        .unwrap();

        let of = |ring: Option<(u32, &Dek)>| {
            vault_status_flags(&s, true, ring)
                .unwrap()
                .ring_is_workspace
        };
        assert!(of(Some((2, &workspace_dek))));
        assert!(!of(Some((2, &Dek::random()))));
        assert!(!of(Some((1, &workspace_dek))));
        assert!(!of(None), "a locked vault seals with nothing");
    }

    /// The status carries the workspace generation raw, so the command can
    /// pair it with the ring — which lives behind a different lock.
    #[test]
    fn the_status_flags_carry_the_workspace_generation() {
        let s = store();
        assert_eq!(
            vault_status_flags(&s, true, None)
                .unwrap()
                .server_generation,
            0
        );
        crate::migrate::set_meta_i64(&s.conn, "vault_generation", 4).unwrap();
        assert_eq!(
            vault_status_flags(&s, true, None)
                .unwrap()
                .server_generation,
            4
        );
    }

    /// The status flags carry the role raw, exactly like `server_generation`
    /// above — `""` before the first pull, whatever the last pull cached
    /// (meta `workspace_role`) afterward.
    #[test]
    fn the_status_flags_carry_the_workspace_role() {
        let s = store();
        assert_eq!(vault_status_flags(&s, true, None).unwrap().role, "");
        crate::migrate::set_meta(&s.conn, "workspace_role", "owner").unwrap();
        assert_eq!(vault_status_flags(&s, true, None).unwrap().role, "owner");
    }

    #[test]
    fn vault_status_flags_read_existence_the_conflict_and_the_recovery_question() {
        let s = store();
        let flags = vault_status_flags(&s, true, None).unwrap();
        assert!(!flags.exists && !flags.conflict && !flags.recovery_holder);

        // The creator of a server-context vault: a record, no cache yet.
        vault_setup(&s, "pw").unwrap();
        crate::migrate::set_meta_i64(&s.conn, "vault_conflict", 1).unwrap();
        let flags = vault_status_flags(&s, true, None).unwrap();
        assert!(flags.exists && flags.conflict && flags.recovery_holder);

        // An invitee's device: cached `mine` only, no record at all.
        let s2 = store();
        let dek = Dek::random();
        s2.set_vault_entries(
            &serde_json::json!({"mine": [my_entry_for(&dek, 2, "pw")], "recovery": []}).to_string(),
        )
        .unwrap();
        let flags = vault_status_flags(&s2, true, None).unwrap();
        assert!(
            flags.exists,
            "the workspace key counts as an existing vault"
        );
        assert!(!flags.conflict && !flags.recovery_holder);
        assert!(
            vault_status_flags(&s2, false, None)
                .unwrap()
                .recovery_holder,
            "a local context is always a holder"
        );
    }

    #[test]
    fn setting_up_again_is_refused_once_the_workspace_holds_a_key_for_this_caller() {
        // The generation-2 case that has no local record to trip over: without
        // this guard `vault_setup` would mint a SECOND, incompatible DEK.
        let s = store();
        let dek = Dek::random();
        s.set_vault_entries(
            &serde_json::json!({"mine": [my_entry_for(&dek, 2, "member-pw")], "recovery": []})
                .to_string(),
        )
        .unwrap();
        assert!(s.vault_record().unwrap().is_none(), "no local record");

        assert_eq!(
            err_of(vault_setup(&s, "another-pw")),
            "vault: a vault already exists"
        );
        assert!(s.vault_record().unwrap().is_none(), "nothing was written");
    }
}

#[cfg(test)]
mod context_vault_info_tests {
    use super::*;

    #[test]
    fn false_on_a_fresh_migrated_db_true_after_setup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.db");
        {
            let s = Store::open(&path).unwrap();
            crate::migrate::run_migrations(&s.conn).unwrap();
        }

        assert_eq!(context_vault_info(&path), ContextVaultInfo::default());

        {
            let s = Store::open(&path).unwrap();
            vault_setup(&s, "hunter2").unwrap();
        }

        assert!(context_vault_info(&path).exists);
    }

    #[test]
    fn the_key_ring_state_comes_from_that_context_s_own_meta() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.db");
        {
            let s = Store::open(&path).unwrap();
            crate::migrate::run_migrations(&s.conn).unwrap();
            vault_setup(&s, "hunter2").unwrap();
            crate::migrate::set_meta_i64(&s.conn, "vault_generation", 3).unwrap();
            crate::migrate::set_meta_i64(&s.conn, "vault_rotation_pending", 1).unwrap();
        }

        assert_eq!(
            context_vault_info(&path),
            ContextVaultInfo {
                exists: true,
                generation: 3,
                rotation_pending: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn a_missing_db_file_reports_no_vault_rather_than_erroring() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does-not-exist.db");
        assert_eq!(context_vault_info(&path), ContextVaultInfo::default());
    }

    #[test]
    fn the_role_and_invites_needing_code_come_from_that_context_s_own_meta() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.db");
        {
            let s = Store::open(&path).unwrap();
            crate::migrate::run_migrations(&s.conn).unwrap();
            crate::migrate::set_meta_i64(&s.conn, "vault_generation", 2).unwrap();
            crate::migrate::set_meta(&s.conn, "workspace_role", "owner").unwrap();
            crate::migrate::set_meta(
                &s.conn,
                "vault_invites",
                r#"[{"invitationId":5,"generation":1},{"invitationId":6,"generation":2}]"#,
            )
            .unwrap();
        }

        let info = context_vault_info(&path);
        assert_eq!(info.role, "owner");
        // Only invitation 5's wrap (generation 1) is stale against the
        // context's own generation 2 — 6's is current.
        assert_eq!(info.invites_needing_code, 1);
    }

    #[test]
    fn a_member_who_joined_after_a_rotation_still_reports_an_existing_vault() {
        // No local record is mirrored for a generation > 1, so reading the
        // record alone would say "no vault" and hide every vault action on
        // that context's row.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.db");
        {
            let s = Store::open(&path).unwrap();
            crate::migrate::run_migrations(&s.conn).unwrap();
            s.set_vault_entries(
                &serde_json::json!({"mine": [my_entry_for(&Dek::random(), 2, "pw")]}).to_string(),
            )
            .unwrap();
            crate::migrate::set_meta_i64(&s.conn, "vault_generation", 2).unwrap();
            assert!(s.vault_record().unwrap().is_none());
        }

        assert_eq!(
            context_vault_info(&path),
            ContextVaultInfo {
                exists: true,
                generation: 2,
                rotation_pending: false,
                ..Default::default()
            }
        );
    }
}

/// Rotating a workspace vault key after a member was removed: the payload the
/// owner uploads, the codes they hand out, what a member's redemption
/// produces, the creator's recovery follow-up, and the lazy re-seal.
#[cfg(test)]
mod vault_rotation_tests {
    use super::test_support::*;
    use super::*;

    /// The cached entries of a vault's CREATOR at generation 1: their own
    /// passphrase wrap plus the workspace recovery wrap.
    fn creator_entries(dek: &Dek, passphrase: &str) -> (VaultEntries, String) {
        let (record, recovery_key, _d) = crate::vault::setup(passphrase).unwrap();
        let entries = VaultEntries {
            mine: vec![MyEntry {
                generation: 1,
                record: crate::vault::rewrap_passphrase(&record, dek, passphrase),
            }],
            recovery: vec![RecoveryEntry {
                generation: 1,
                recovery_salt: record.recovery_salt,
                dek_wrapped_recovery: record.dek_wrapped_recovery.clone(),
                dek_check: record.dek_check.clone(),
            }],
            rotation: vec![],
        };
        (entries, recovery_key.as_str().to_string())
    }

    #[test]
    fn the_cache_carries_rotation_wraps_and_older_caches_still_parse() {
        let dek = Dek::random();
        let (_code, wrap) = make_invite_wrap(&dek, 2);
        let json = serde_json::json!({
            "mine": [my_entry_for(&dek, 1, "pw")],
            "recovery": [],
            "rotation": [{
                "generation": 2,
                "kdfParams": serde_json::to_value(&wrap.kdf_params).unwrap(),
                "dekWrapped": wrap.dek_wrapped,
                "dekCheck": wrap.dek_check,
            }],
        })
        .to_string();
        let entries = VaultEntries::from_json(&json).unwrap();
        assert_eq!(entries.rotation.len(), 1);
        assert_eq!(pending_rotation_generations(&entries), vec![2]);

        // Round-trips through the cache unchanged...
        let back = VaultEntries::from_json(&entries.to_json()).unwrap();
        assert_eq!(pending_rotation_generations(&back), vec![2]);
        // ...and a cache written before rotation wraps existed still parses.
        let old = VaultEntries::from_json(r#"{"mine":[],"recovery":[]}"#).unwrap();
        assert!(old.rotation.is_empty());
    }

    /// The rotation's bookkeeping, out of the command body and into a place
    /// a test can reach.
    #[test]
    fn a_rotation_plan_excludes_the_caller_and_steps_the_generation() {
        let plan = rotation_plan(2, &[7, 3, 9, 3], 7);
        assert_eq!(plan.new_generation, 3);
        assert_eq!(plan.others, vec![3, 9], "no self, no duplicates, ascending");

        // A workspace with no vault yet (generation 0) rotates to 1.
        assert_eq!(rotation_plan(0, &[7], 7).new_generation, 1);
        assert!(rotation_plan(0, &[7], 7).others.is_empty(), "a lone member");

        // A member the listing does not include cannot be excluded twice.
        assert_eq!(rotation_plan(1, &[3, 9], 7).others, vec![3, 9]);

        // Saturating, never wrapping: rolling back to a generation an
        // ex-member still knows the key for would undo the whole rotation.
        assert_eq!(rotation_plan(u32::MAX, &[7], 7).new_generation, u32::MAX);
    }

    #[test]
    fn rotation_payload_wraps_the_owner_by_passphrase_and_everyone_else_by_code() {
        let new_dek = Dek::random();
        let (payload, codes) = rotation_payload(
            2,
            &new_dek,
            (7, "owner-pw"),
            &[7, 8, 9], // the listing includes the caller
            None,
        )
        .unwrap();

        assert_eq!(payload.generation, 2);
        assert!(payload.recovery.is_none());
        assert_eq!(payload.keys.len(), 3, "one row per member, the caller once");
        let own = payload.keys.iter().find(|k| k.kind == "own").unwrap();
        assert_eq!(own.user_id, 7);
        assert_eq!(
            codes.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            vec![8, 9]
        );

        // The owner's own row opens with their passphrase...
        let mine = VaultEntries {
            mine: vec![MyEntry::try_from(payload.own_entry().unwrap()).unwrap()],
            recovery: vec![],
            rotation: vec![],
        };
        assert_eq!(
            unlock_entries_with_passphrase(&mine, "owner-pw").unwrap()[0]
                .1
                .expose(),
            new_dek.expose()
        );
        // ...and every other row only with that member's own code.
        for (user_id, code) in &codes {
            let k = payload.keys.iter().find(|k| k.user_id == *user_id).unwrap();
            let wrap = InviteWrap {
                generation: 2,
                kdf_params: k.kdf_params.clone(),
                dek_wrapped: k.dek_wrapped.clone(),
                dek_check: k.dek_check.clone(),
            };
            assert_eq!(
                open_invite_wrap(&wrap, code).unwrap().expose(),
                new_dek.expose()
            );
            let other = codes.iter().find(|(id, _)| id != user_id).unwrap();
            assert_eq!(
                err_of(open_invite_wrap(&wrap, &other.1)),
                "invalid invite code",
                "a member's code opens only their own wrap"
            );
        }
    }

    #[test]
    fn the_creator_rotating_carries_the_recovery_wrap_for_the_new_generation() {
        let new_dek = Dek::random();
        let (_e, recovery_key) = creator_entries(&Dek::random(), "owner-pw");
        let (payload, _codes) =
            rotation_payload(3, &new_dek, (1, "owner-pw"), &[], Some(&recovery_key)).unwrap();
        let recovery = payload.recovery.expect("the creator supplies it inline");

        let entries = merge_recovery_entry(&VaultEntries::default(), 3, &recovery).unwrap();
        let opened = unlock_entries_with_recovery(&entries, &recovery_key).unwrap();
        assert_eq!(opened[0].0, 3);
        assert_eq!(opened[0].1.expose(), new_dek.expose());
        assert_eq!(
            err_of(unlock_entries_with_recovery(&entries, "AAAAA-BBBBB-CCCCC")),
            "wrong recovery key"
        );
    }

    #[test]
    fn a_member_redeems_their_code_into_a_wrap_under_their_own_passphrase() {
        let old_dek = Dek::random();
        let new_dek = Dek::random();
        let (code, wrap) = make_invite_wrap(&new_dek, 2);
        let entries = VaultEntries {
            mine: vec![MyEntry::try_from(my_entry_for(&old_dek, 1, "member-pw")).unwrap()],
            recovery: vec![],
            rotation: vec![MyEntry::try_from(MyEntryWire {
                generation: 2,
                kdf_params: wrap.kdf_params.clone(),
                dek_wrapped: wrap.dek_wrapped.clone(),
                dek_check: wrap.dek_check.clone(),
            })
            .unwrap()],
        };

        // The passphrase is checked against the newest wrap the member holds.
        assert!(verify_newest_passphrase(&entries, "member-pw").is_ok());
        assert_eq!(
            err_of(verify_newest_passphrase(&entries, "nope")),
            "wrong passphrase"
        );

        let redeemed = rotation_redeem_entries(&entries, &code, "member-pw").unwrap();
        assert_eq!(redeemed.len(), 1);
        let (generation, dek, own) = &redeemed[0];
        assert_eq!(*generation, 2);
        assert_eq!(dek.expose(), new_dek.expose());

        // Caching the member's own wrap spends the rotation entry.
        let merged = merge_my_entry(&entries, own.clone()).unwrap();
        assert!(merged.rotation.is_empty());
        assert!(pending_rotation_generations(&merged).is_empty());
        let opened = unlock_entries_with_passphrase(&merged, "member-pw").unwrap();
        assert_eq!(
            opened.iter().map(|(g, _)| *g).collect::<Vec<_>>(),
            vec![1, 2]
        );
    }

    /// A pending rotation wrap for `generation`, as the server parks it.
    fn rotation_entry(dek: &Dek, generation: u32) -> (String, MyEntry) {
        let (code, wrap) = make_invite_wrap(dek, generation);
        let entry = MyEntry::try_from(MyEntryWire {
            generation,
            kdf_params: wrap.kdf_params,
            dek_wrapped: wrap.dek_wrapped,
            dek_check: wrap.dek_check,
        })
        .unwrap();
        (code, entry)
    }

    #[test]
    fn two_rotations_missed_are_redeemed_one_code_at_a_time() {
        // The member was offline across two removals, so the workspace parked
        // TWO wraps for them behind two different codes. Neither code opens
        // the other's wrap — failing the whole redemption on the first
        // mismatch would make both codes useless.
        let (d2, d3) = (Dek::random(), Dek::random());
        let (code2, rot2) = rotation_entry(&d2, 2);
        let (code3, rot3) = rotation_entry(&d3, 3);
        let entries = VaultEntries {
            mine: vec![MyEntry::try_from(my_entry_for(&Dek::random(), 1, "member-pw")).unwrap()],
            recovery: vec![],
            rotation: vec![rot2, rot3],
        };
        assert_eq!(pending_rotation_generations(&entries), vec![2, 3]);

        // The newer code redeems only generation 3...
        let redeemed = rotation_redeem_entries(&entries, &code3, "member-pw").unwrap();
        assert_eq!(redeemed.len(), 1);
        assert_eq!(redeemed[0].0, 3);
        assert_eq!(redeemed[0].1.expose(), d3.expose());
        let after = merge_my_entry(&entries, redeemed[0].2.clone()).unwrap();
        assert_eq!(
            pending_rotation_generations(&after),
            vec![2],
            "generation 2 stays pending for its own code"
        );

        // ...and the older code then redeems generation 2 from that state.
        let redeemed = rotation_redeem_entries(&after, &code2, "member-pw").unwrap();
        assert_eq!(redeemed.len(), 1);
        assert_eq!(redeemed[0].0, 2);
        assert_eq!(redeemed[0].1.expose(), d2.expose());
        let done = merge_my_entry(&after, redeemed[0].2.clone()).unwrap();
        assert!(pending_rotation_generations(&done).is_empty());
        assert_eq!(
            unlock_entries_with_passphrase(&done, "member-pw")
                .unwrap()
                .iter()
                .map(|(g, _)| *g)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn a_code_that_opens_none_of_the_pending_wraps_changes_nothing() {
        let (_c2, rot2) = rotation_entry(&Dek::random(), 2);
        let (_c3, rot3) = rotation_entry(&Dek::random(), 3);
        let entries = VaultEntries {
            mine: vec![MyEntry::try_from(my_entry_for(&Dek::random(), 1, "member-pw")).unwrap()],
            recovery: vec![],
            rotation: vec![rot2, rot3],
        };
        assert_eq!(
            err_of(rotation_redeem_entries(
                &entries,
                "AAAA-AAAA-AAAA-AAAA-AAAA-AAAA-AA",
                "member-pw"
            )),
            "invalid rotation code"
        );
        assert_eq!(
            pending_rotation_generations(&entries),
            vec![2, 3],
            "both stay pending"
        );
    }

    #[test]
    fn a_wrong_rotation_code_is_refused_and_nothing_pending_is_an_error() {
        let new_dek = Dek::random();
        let (_code, wrap) = make_invite_wrap(&new_dek, 2);
        let entries = VaultEntries {
            mine: vec![MyEntry::try_from(my_entry_for(&Dek::random(), 1, "pw")).unwrap()],
            recovery: vec![],
            rotation: vec![MyEntry::try_from(MyEntryWire {
                generation: 2,
                kdf_params: wrap.kdf_params,
                dek_wrapped: wrap.dek_wrapped,
                dek_check: wrap.dek_check,
            })
            .unwrap()],
        };
        assert_eq!(
            err_of(rotation_redeem_entries(
                &entries,
                "AAAA-AAAA-AAAA-AAAA-AAAA-AAAA-AA",
                "pw"
            )),
            "invalid rotation code"
        );
        assert_eq!(
            err_of(rotation_redeem_entries(
                &VaultEntries::default(),
                "AAAA",
                "pw"
            )),
            "no rotation pending"
        );
    }

    #[test]
    fn the_creator_fills_in_the_recovery_wrap_a_foreign_rotation_left_out() {
        let d1 = Dek::random();
        let d2 = Dek::random();
        let (base, recovery_key) = creator_entries(&d1, "owner-pw");
        // Someone else rotated: generation 2 has a wrap for us, but no
        // recovery wrap anywhere.
        let entries = merge_my_entry(&base, my_entry_for(&d2, 2, "owner-pw")).unwrap();
        assert_eq!(generations_missing_recovery(&entries), vec![2]);

        assert_eq!(
            err_of(recovery_followup(&entries, &[], "AAAAA-BBBBB-CCCCC")),
            "wrong recovery key"
        );
        // A generation the ring cannot open is skipped rather than failing.
        assert!(recovery_followup(&entries, &[], &recovery_key)
            .unwrap()
            .is_empty());

        let deks = vec![(1, d1.clone()), (2, d2.clone())];
        let payloads = recovery_followup(&entries, &deks, &recovery_key).unwrap();
        assert_eq!(payloads.len(), 1);
        let (generation, payload) = &payloads[0];
        assert_eq!(*generation, 2);

        let filled = merge_recovery_entry(&entries, *generation, payload).unwrap();
        assert!(generations_missing_recovery(&filled).is_empty());
        let opened = unlock_entries_with_recovery(&filled, &recovery_key).unwrap();
        assert_eq!(
            opened.iter().map(|(g, _)| *g).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(opened[1].1.expose(), d2.expose());
    }

    #[test]
    fn recovery_create_payloads_cover_every_generation_and_open_with_the_key() {
        let (d1, d2) = (Dek::random(), Dek::random());
        let key = crate::vault::recovery::RecoveryKey::generate();
        let payloads =
            recovery_create_payloads(&[(1, d1.clone()), (2, d2.clone())], key.as_str()).unwrap();
        assert_eq!(
            payloads.iter().map(|(g, _)| *g).collect::<Vec<_>>(),
            vec![1, 2]
        );
        // Merged the way the command caches them, they open with the key.
        let mut entries = VaultEntries::default();
        for (g, p) in &payloads {
            entries = merge_recovery_entry(&entries, *g, p).unwrap();
        }
        let mut opened = unlock_entries_with_recovery(&entries, key.as_str()).unwrap();
        opened.sort_by_key(|(g, _)| *g);
        assert_eq!(opened[0].1.expose(), d1.expose());
        assert_eq!(opened[1].1.expose(), d2.expose());
    }

    fn mint_flags(
        conflict: bool,
        ring_is_workspace: bool,
        server_generation: i64,
    ) -> VaultStatusFlags {
        VaultStatusFlags {
            exists: true,
            conflict,
            recovery_holder: false,
            rotation_code: false,
            recovery_missing: false,
            server_generation,
            ring_is_workspace,
            role: "owner".into(),
        }
    }

    #[test]
    fn invite_wrap_allowed_refuses_a_conflicted_or_outdated_ring() {
        assert!(invite_wrap_allowed(&mint_flags(false, true, 2), 2).is_ok());
        // Conflicted, but the ring's newest key IS the workspace's: fine.
        assert!(invite_wrap_allowed(&mint_flags(true, true, 2), 2).is_ok());
        assert_eq!(
            invite_wrap_allowed(&mint_flags(true, false, 2), 2).unwrap_err(),
            "vault: resolve the vault conflict first"
        );
        assert_eq!(
            invite_wrap_allowed(&mint_flags(false, true, 3), 2).unwrap_err(),
            "vault: redeem the rotation code first"
        );
        // A local context reports generation 0: never outdated.
        assert!(invite_wrap_allowed(&mint_flags(false, false, 0), 1).is_ok());
    }

    #[test]
    fn recovery_eligible_needs_an_unlocked_owner_without_a_set() {
        assert!(recovery_eligible(true, "owner", false, true, false));
        assert!(!recovery_eligible(true, "owner", true, true, false));
        assert!(!recovery_eligible(true, "editor", false, true, false));
        assert!(!recovery_eligible(true, "owner", false, false, false));
        assert!(!recovery_eligible(false, "owner", false, true, false));
        assert!(
            !recovery_eligible(true, "owner", false, true, true),
            "a device holding two vaults' keys must never upload a wrap of its \
             local key under a workspace generation"
        );
    }

    #[test]
    fn the_status_flags_surface_a_pending_code_and_a_missing_recovery_wrap() {
        let s = store();
        let d1 = Dek::random();
        let (base, _rk) = creator_entries(&d1, "owner-pw");

        // The creator, everything in order.
        s.set_vault_entries(&base.to_json()).unwrap();
        let flags = vault_status_flags(&s, true, None).unwrap();
        assert!(!flags.rotation_code && !flags.recovery_missing);

        // Someone else rotated: our own wrap for generation 2 arrived, its
        // recovery wrap did not.
        let with_gen2 = merge_my_entry(&base, my_entry_for(&Dek::random(), 2, "owner-pw")).unwrap();
        s.set_vault_entries(&with_gen2.to_json()).unwrap();
        let flags = vault_status_flags(&s, true, None).unwrap();
        assert!(flags.recovery_missing && !flags.rotation_code);
        assert!(
            !vault_status_flags(&s, false, None)
                .unwrap()
                .recovery_missing,
            "a local context has no workspace recovery wraps to fill in"
        );

        // A member waiting to redeem their rotation code.
        let s2 = store();
        let (_code, wrap) = make_invite_wrap(&Dek::random(), 2);
        s2.set_vault_entries(
            &serde_json::json!({
                "mine": [my_entry_for(&d1, 1, "member-pw")],
                "recovery": [],
                "rotation": [{
                    "generation": 2,
                    "kdfParams": serde_json::to_value(&wrap.kdf_params).unwrap(),
                    "dekWrapped": wrap.dek_wrapped,
                    "dekCheck": wrap.dek_check,
                }],
            })
            .to_string(),
        )
        .unwrap();
        let flags = vault_status_flags(&s2, true, None).unwrap();
        assert!(flags.rotation_code && !flags.recovery_missing);
    }

    #[test]
    fn reseal_moves_lagging_notes_to_the_newest_generation_in_batches() {
        let mut s = Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        s.sync_enabled = true;
        let (d1, d2) = (Dek::random(), Dek::random());
        let mut vault = VaultState::default();
        vault.unlock(1, d1.clone());
        for id in ["a", "b", "c"] {
            s.save_note(&Note {
                id: id.into(),
                content: format!("<p>{id}</p>"),
                updated_at: 1,
                ..Default::default()
            })
            .unwrap();
            s.set_note_protected(id, true).unwrap();
            encrypt_note_in_place(&s, id, &d1, 1).unwrap();
        }
        let ts: Vec<_> = s
            .load_dirty_notes()
            .unwrap()
            .iter()
            .map(|n| (n.id.clone(), n.updated_at))
            .collect();
        s.clear_note_dirty(&ts).unwrap();
        vault.unlock(2, d2.clone());
        assert_eq!(reseal_lagging_notes(&s, &vault, 2).unwrap(), 2);
        assert_eq!(reseal_lagging_notes(&s, &vault, 2).unwrap(), 1);
        assert_eq!(reseal_lagging_notes(&s, &vault, 2).unwrap(), 0);
        for id in ["a", "b", "c"] {
            assert_eq!(s.note_key_gen(id).unwrap(), Some(2));
            assert_eq!(
                open_note_content(&s, &vault, id).unwrap(),
                format!("<p>{id}</p>")
            );
        }
        assert_eq!(
            s.load_dirty_notes().unwrap().len(),
            3,
            "resealed notes are pushed"
        );
        let mut only_old = VaultState::default();
        only_old.unlock(1, d1);
        assert!(
            open_note_content(&s, &only_old, "a").is_err(),
            "sealed under the new generation now"
        );
    }

    #[test]
    fn notes_predating_the_generation_column_are_only_stamped_never_re_sealed() {
        // Schema v15 added `key_gen` without backfilling it, and NULL reads as
        // generation 1. At newest = 1 those rows are NOT lagging: re-sealing
        // them would burn a new nonce on the same key, dirty the row and move
        // its "last edited" date for nothing.
        let mut s = Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        s.sync_enabled = true;
        let d1 = Dek::random();
        let mut vault = VaultState::default();
        vault.unlock(1, d1.clone());
        for id in ["a", "b", "c"] {
            s.save_note(&Note {
                id: id.into(),
                content: format!("<p>{id}</p>"),
                updated_at: 1,
                ..Default::default()
            })
            .unwrap();
            s.set_note_protected(id, true).unwrap();
            encrypt_note_in_place(&s, id, &d1, 1).unwrap();
            s.set_note_key_gen(id, None).unwrap(); // the pre-v15 state
        }
        let ts: Vec<_> = s
            .load_dirty_notes()
            .unwrap()
            .iter()
            .map(|n| (n.id.clone(), n.updated_at))
            .collect();
        s.clear_note_dirty(&ts).unwrap();
        let updated_at_of = |store: &Store, id: &str| -> i64 {
            store
                .load_notes()
                .unwrap()
                .into_iter()
                .find(|n| n.id == id)
                .expect("the note is there")
                .updated_at
        };
        let before: Vec<_> = ["a", "b", "c"]
            .iter()
            .map(|id| {
                (
                    s.load_note_content(id).unwrap().unwrap(),
                    updated_at_of(&s, id),
                )
            })
            .collect();

        assert_eq!(reseal_lagging_notes(&s, &vault, 25).unwrap(), 3);

        for (i, id) in ["a", "b", "c"].iter().enumerate() {
            assert_eq!(s.note_key_gen(id).unwrap(), Some(1), "stamped");
            assert_eq!(
                s.load_note_content(id).unwrap().unwrap(),
                before[i].0,
                "ciphertext byte-identical — not re-sealed"
            );
            assert_eq!(
                updated_at_of(&s, id),
                before[i].1,
                "the note was not edited"
            );
            assert_eq!(
                open_note_content(&s, &vault, id).unwrap(),
                format!("<p>{id}</p>")
            );
        }
        assert!(
            s.load_dirty_notes().unwrap().is_empty(),
            "nothing to push — the sweep changed no content"
        );
        // The work list drained, so the next cycle does nothing at all.
        assert_eq!(reseal_lagging_notes(&s, &vault, 25).unwrap(), 0);
    }

    #[test]
    fn reseal_skips_a_locked_vault_and_generations_this_ring_cannot_open() {
        let s = store();
        let d1 = Dek::random();
        seed(&s, "a", "<p>a</p>");
        s.set_note_protected("a", true).unwrap();
        encrypt_note_in_place(&s, "a", &d1, 1).unwrap();

        assert_eq!(
            reseal_lagging_notes(&s, &VaultState::default(), 10).unwrap(),
            0,
            "locked: nothing to seal with"
        );

        // A member who joined at generation 2 never saw generation 1's DEK.
        let mut only_new = VaultState::default();
        only_new.unlock(2, Dek::random());
        assert_eq!(reseal_lagging_notes(&s, &only_new, 10).unwrap(), 0);
        assert_eq!(s.note_key_gen("a").unwrap(), Some(1), "left untouched");
    }

    /// C1(a): a conflicted device holds two vaults' keys at once. Its own
    /// notes must never move under the workspace's newer generation — that
    /// would hand every workspace member the private notes it protected
    /// before joining.
    #[test]
    fn reseal_stands_down_entirely_on_a_conflicted_device() {
        let s = store();
        let local_dek = Dek::random();
        seed(&s, "private", "<p>mine</p>");
        s.set_note_protected("private", true).unwrap();
        encrypt_note_in_place(&s, "private", &local_dek, 1).unwrap();
        let sealed = content_of(&s, "private");

        // The ring: this device's own generation 1 plus the workspace's
        // generation 2. Without the flag the sweep would happily re-seal.
        let ws_dek = Dek::random();
        let mut vault = VaultState::default();
        vault.unlock(1, local_dek.clone());
        vault.unlock(2, ws_dek);

        crate::migrate::set_meta_i64(&s.conn, "vault_conflict", 1).unwrap();
        assert_eq!(reseal_lagging_notes(&s, &vault, 10).unwrap(), 0);
        assert_eq!(s.note_key_gen("private").unwrap(), Some(1));
        assert_eq!(content_of(&s, "private"), sealed, "ciphertext untouched");

        // Cleared (an unlock proved the two are one vault): the sweep runs.
        crate::migrate::delete_meta(&s.conn, "vault_conflict").unwrap();
        assert_eq!(reseal_lagging_notes(&s, &vault, 10).unwrap(), 1);
        assert_eq!(s.note_key_gen("private").unwrap(), Some(2));
    }

    #[test]
    fn a_cache_only_device_is_told_to_unlock_rather_than_set_up() {
        let s = store();
        assert!(!server_vault_needs_unlock(&s).unwrap());

        s.set_vault_entries(
            &serde_json::json!({"mine": [my_entry_for(&Dek::random(), 2, "pw")]}).to_string(),
        )
        .unwrap();
        assert!(server_vault_needs_unlock(&s).unwrap());

        // A device with a record of its own is not in that situation.
        let s2 = store();
        vault_setup(&s2, "pw").unwrap();
        assert!(!server_vault_needs_unlock(&s2).unwrap());
    }
}

#[cfg(test)]
mod context_vault_change_passphrase_tests {
    use super::test_support::*;
    use super::*;

    struct Fixture {
        _dir: tempfile::TempDir,
        reg: Registry,
        other_path: PathBuf,
    }

    /// A registry with the default (active) context plus a second, NOT
    /// active, context entry pointing at its own real DB file — the shape
    /// `change_context_vault_passphrase` is meant to operate on.
    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let mut reg = Registry::default_for(&dir.path().join("active.db").to_string_lossy());
        let other_path = dir.path().join("other.db");
        reg.add(
            "other".into(),
            "Other".into(),
            other_path.to_string_lossy().into_owned(),
        );
        Fixture {
            _dir: dir,
            reg,
            other_path,
        }
    }

    fn setup_vault_on(path: &Path, passphrase: &str) -> Vec<String> {
        let s = Store::open(path).unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        let (groups, _dek) = vault_setup(&s, passphrase).unwrap();
        groups
    }

    fn reopen_other(f: &Fixture) -> Store {
        Store::open(&f.other_path).unwrap()
    }

    #[test]
    fn rewraps_the_non_active_contexts_vault() {
        let f = fixture();
        setup_vault_on(&f.other_path, "old");

        change_context_vault_passphrase(&f.reg, "other", "old", "new").unwrap();

        let s = reopen_other(&f);
        assert!(vault_unlock_passphrase(&record(&s), "old").is_err());
        assert!(vault_unlock_passphrase(&record(&s), "new").is_ok());
    }

    #[test]
    fn the_rewrapped_record_carries_a_dek_check() {
        let f = fixture();
        setup_vault_on(&f.other_path, "old");

        change_context_vault_passphrase(&f.reg, "other", "old", "new").unwrap();

        let s = reopen_other(&f);
        assert!(record(&s).dek_check.is_some());
    }

    #[test]
    fn the_recovery_key_keeps_working_after_the_change() {
        let f = fixture();
        let groups = setup_vault_on(&f.other_path, "old");

        change_context_vault_passphrase(&f.reg, "other", "old", "new").unwrap();

        let s = reopen_other(&f);
        assert!(vault_unlock_recovery(&record(&s), &groups.join("-")).is_ok());
    }

    #[test]
    fn an_unknown_context_id_is_rejected() {
        let f = fixture();
        assert_eq!(
            err_of(change_context_vault_passphrase(
                &f.reg, "nope", "old", "new"
            )),
            "unknown context"
        );
    }

    #[test]
    fn the_active_context_is_refused_here() {
        let f = fixture();
        let active_id = f.reg.active_id.clone();
        assert_eq!(
            err_of(change_context_vault_passphrase(
                &f.reg, &active_id, "old", "new"
            )),
            "active context: use vault_change_passphrase"
        );
    }

    #[test]
    fn a_wrong_current_passphrase_leaves_the_other_contexts_vault_untouched() {
        let f = fixture();
        setup_vault_on(&f.other_path, "old");

        assert!(change_context_vault_passphrase(&f.reg, "other", "wrong", "new").is_err());

        let s = reopen_other(&f);
        assert!(vault_unlock_passphrase(&record(&s), "old").is_ok());
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

        set_note_protected(&s, &unlocked_at(1, dek), "n1", true).unwrap();

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
        let vault = unlocked_at(1, dek.clone());
        set_note_protected(&s, &vault, "n1", true).unwrap();
        let sealed = content_of(&s, "n1");

        set_note_protected(&s, &vault, "n1", true).unwrap();

        assert_eq!(content_of(&s, "n1"), sealed, "no double encryption");
        assert_eq!(open_content(&dek, "n1", &sealed).unwrap(), "<p>secret</p>");
    }

    #[test]
    fn unprotecting_restores_the_plaintext() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>secret</p>");
        let vault = unlocked_at(1, dek);
        set_note_protected(&s, &vault, "n1", true).unwrap();

        set_note_protected(&s, &vault, "n1", false).unwrap();

        assert!(!s.note_protected("n1").unwrap());
        assert_eq!(content_of(&s, "n1"), "<p>secret</p>");
        assert_eq!(
            s.note_key_gen("n1").unwrap(),
            None,
            "the generation marker is cleared along with `protected`"
        );
    }

    #[test]
    fn unprotecting_an_unprotected_note_is_a_no_op() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>plain</p>");

        set_note_protected(&s, &unlocked_at(1, dek), "n1", false).unwrap();

        assert_eq!(content_of(&s, "n1"), "<p>plain</p>");
        assert!(!s.note_protected("n1").unwrap());
    }

    #[test]
    fn unprotecting_is_refused_while_the_note_sits_in_a_locked_folder() {
        let s = store();
        let dek = Dek::random();
        folder(&s, "f", None);
        seed_in(&s, "n1", "<p>secret</p>", "f");
        let vault = unlocked_at(1, dek);
        set_note_protected(&s, &vault, "n1", true).unwrap();
        s.set_folder_locked("f", true).unwrap();
        let sealed = content_of(&s, "n1");

        let err = set_note_protected(&s, &vault, "n1", false).unwrap_err();

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
        let vault = unlocked_at(1, dek);
        set_note_protected(&s, &vault, "n1", true).unwrap();
        s.set_folder_locked("top", true).unwrap();

        assert_eq!(
            set_note_protected(&s, &vault, "n1", false).unwrap_err(),
            "note is protected by its folder"
        );
    }

    #[test]
    fn unprotecting_with_a_foreign_dek_fails_and_keeps_the_ciphertext() {
        let s = store();
        let dek = Dek::random();
        seed(&s, "n1", "<p>secret</p>");
        set_note_protected(&s, &unlocked_at(1, dek), "n1", true).unwrap();
        let sealed = content_of(&s, "n1");

        // Same generation (1) as the note was sealed under, but a DIFFERENT
        // DEK — the ring has an entry, so this is a decrypt/authentication
        // failure, not "key generation not available".
        assert!(set_note_protected(&s, &unlocked_at(1, Dek::random()), "n1", false).is_err());

        assert!(s.note_protected("n1").unwrap());
        assert_eq!(
            content_of(&s, "n1"),
            sealed,
            "a failed decrypt must never blank or corrupt the stored blob"
        );
    }

    #[test]
    fn unprotecting_opens_under_the_notes_own_generation_not_the_rings_newest() {
        // Sealed while generation 1 was the ring's newest.
        let s = store();
        let d1 = Dek::random();
        seed(&s, "n1", "<p>secret</p>");
        set_note_protected(&s, &unlocked_at(1, d1.clone()), "n1", true).unwrap();
        assert_eq!(s.note_key_gen("n1").unwrap(), Some(1));

        // The vault has since rotated: the ring now holds BOTH generations,
        // with 2 as the newest. Unprotecting must still open under the
        // note's OWN generation (1), not reach for the newest (2).
        let mut ring = VaultState::default();
        ring.unlock(1, d1);
        ring.unlock(2, Dek::random());

        set_note_protected(&s, &ring, "n1", false).unwrap();

        assert!(!s.note_protected("n1").unwrap());
        assert_eq!(content_of(&s, "n1"), "<p>secret</p>");
        assert_eq!(s.note_key_gen("n1").unwrap(), None);
    }

    #[test]
    fn unprotecting_fails_when_the_ring_lacks_the_notes_generation() {
        let s = store();
        let d1 = Dek::random();
        seed(&s, "n1", "<p>secret</p>");
        set_note_protected(&s, &unlocked_at(1, d1), "n1", true).unwrap();

        // The ring is unlocked, but only at a generation OTHER than the one
        // this note was sealed under — distinct from a foreign-key decrypt
        // failure (the ring has NO entry for generation 1 at all here).
        let only_new = unlocked_at(2, Dek::random());
        let err = set_note_protected(&s, &only_new, "n1", false).unwrap_err();

        assert_eq!(err, "key generation not available");
        assert!(
            s.note_protected("n1").unwrap(),
            "still protected — nothing was committed"
        );
    }

    #[test]
    fn protecting_a_missing_note_is_an_error() {
        let s = store();
        let vault = unlocked_at(1, Dek::random());
        assert!(set_note_protected(&s, &vault, "ghost", true).is_err());
        assert!(set_note_protected(&s, &vault, "ghost", false).is_err());
    }

    #[test]
    fn a_protect_round_trip_is_lossless_for_unicode_and_markup() {
        let s = store();
        let dek = Dek::random();
        let body = "<p>Grüße 🌍</p><p>&lt;escaped&gt;</p>";
        seed(&s, "n1", body);
        let vault = unlocked_at(1, dek);

        set_note_protected(&s, &vault, "n1", true).unwrap();
        set_note_protected(&s, &vault, "n1", false).unwrap();

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

        set_folder_locked(&s, &unlocked_at(1, dek), "top", true).unwrap();

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

        set_folder_locked(&s, &unlocked_at(1, dek), "f", true).unwrap();

        assert_eq!(revision_count(&s, "n1"), 0);
    }

    #[test]
    fn locking_leaves_an_already_encrypted_note_untouched() {
        let s = store();
        let dek = Dek::random();
        folder(&s, "f", None);
        seed_in(&s, "n1", "<p>secret</p>", "f");
        encrypt_note_in_place(&s, "n1", &dek, 1).unwrap();
        let sealed = content_of(&s, "n1");

        set_folder_locked(&s, &unlocked_at(1, dek), "f", true).unwrap();

        assert_eq!(content_of(&s, "n1"), sealed, "no double encryption");
    }

    #[test]
    fn unlocking_decrypts_the_subtree_again() {
        let s = store();
        let dek = Dek::random();
        folder(&s, "f", None);
        seed_in(&s, "n1", "<p>secret</p>", "f");
        let vault = unlocked_at(1, dek);
        set_folder_locked(&s, &vault, "f", true).unwrap();

        set_folder_locked(&s, &vault, "f", false).unwrap();

        assert!(!s.folder_locked("f").unwrap());
        assert!(!s.note_protected("n1").unwrap());
        assert_eq!(content_of(&s, "n1"), "<p>secret</p>");
        assert_eq!(s.note_key_gen("n1").unwrap(), None);
    }

    #[test]
    fn unlocking_keeps_notes_sealed_while_another_ancestor_stays_locked() {
        let s = store();
        let dek = Dek::random();
        folder(&s, "top", None);
        folder(&s, "sub", Some("top"));
        seed_in(&s, "n1", "<p>secret</p>", "sub");
        let vault = unlocked_at(1, dek);
        set_folder_locked(&s, &vault, "top", true).unwrap();
        let sealed = content_of(&s, "n1");

        set_folder_locked(&s, &vault, "sub", false).unwrap();

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

        set_folder_locked(&s, &unlocked_at(1, dek), "f", true).unwrap();

        assert!(s.folder_locked("f").unwrap());
    }

    #[test]
    fn locking_marks_the_subtrees_notes_dirty_when_syncing() {
        let s = syncing_store();
        let dek = Dek::random();
        folder(&s, "f", None);
        seed_in(&s, "n1", "<p>secret</p>", "f");
        clear_dirty(&s);

        set_folder_locked(&s, &unlocked_at(1, dek), "f", true).unwrap();

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
        set_folder_locked(&s, &unlocked_at(1, dek), "f", true).unwrap();
        let sealed = content_of(&s, "n1");

        // Same generation (1), wrong DEK: the ring has an entry, so the
        // per-note generation pre-check passes, and this fails only once the
        // actual decrypt authenticates against the wrong key.
        assert!(set_folder_locked(&s, &unlocked_at(1, Dek::random()), "f", false).is_err());

        assert!(s.note_protected("n1").unwrap());
        assert_eq!(content_of(&s, "n1"), sealed);
    }

    #[test]
    fn unlocking_opens_notes_under_their_own_generation_not_the_rings_newest() {
        let s = store();
        let d1 = Dek::random();
        folder(&s, "f", None);
        seed_in(&s, "n1", "<p>secret</p>", "f");
        set_folder_locked(&s, &unlocked_at(1, d1.clone()), "f", true).unwrap();
        assert_eq!(s.note_key_gen("n1").unwrap(), Some(1));

        // Rotated since: the ring now holds both generations, newest = 2.
        let mut ring = VaultState::default();
        ring.unlock(1, d1);
        ring.unlock(2, Dek::random());

        set_folder_locked(&s, &ring, "f", false).unwrap();

        assert!(!s.folder_locked("f").unwrap());
        assert!(!s.note_protected("n1").unwrap());
        assert_eq!(content_of(&s, "n1"), "<p>secret</p>");
    }

    #[test]
    fn unlocking_fails_and_leaves_the_folder_locked_when_the_ring_lacks_a_notes_generation() {
        let s = store();
        let d1 = Dek::random();
        folder(&s, "f", None);
        seed_in(&s, "n1", "<p>secret</p>", "f");
        set_folder_locked(&s, &unlocked_at(1, d1), "f", true).unwrap();

        // Unlocked, but only at a generation OTHER than the one this note
        // was sealed under — the ring has NO entry for generation 1 at all.
        let only_new = unlocked_at(2, Dek::random());
        let err = set_folder_locked(&s, &only_new, "f", false).unwrap_err();

        assert_eq!(err, "key generation not available");
        assert!(
            s.folder_locked("f").unwrap(),
            "the folder must stay locked — nothing was committed"
        );
        assert!(
            s.note_protected("n1").unwrap(),
            "the note must stay sealed — nothing was committed"
        );
    }

    /// C1(d): on a conflicted device the ring can carry a generation number
    /// whose DEK belongs to the OTHER vault — `dek_for` answers `Some` and
    /// the AEAD open still fails. Trial-opening every note up front is what
    /// stops the folder from being committed open over sealed notes.
    #[test]
    fn unlocking_fails_when_the_ring_holds_a_foreign_dek_for_the_notes_generation() {
        let s = store();
        let d1 = Dek::random();
        folder(&s, "f", None);
        seed_in(&s, "n1", "<p>secret</p>", "f");
        set_folder_locked(&s, &unlocked_at(1, d1), "f", true).unwrap();
        let sealed = content_of(&s, "n1");

        // Generation 1 IS in the ring — but it is another vault's key.
        let impostor = unlocked_at(1, Dek::random());
        let err = set_folder_locked(&s, &impostor, "f", false).unwrap_err();

        assert_eq!(err, "key generation not available");
        assert!(s.folder_locked("f").unwrap(), "folder stays locked");
        assert!(s.note_protected("n1").unwrap(), "note stays sealed");
        assert_eq!(content_of(&s, "n1"), sealed, "ciphertext untouched");
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
    fn to_infos_defaults_the_vault_flags_to_false() {
        let r = registry();
        assert!(to_infos(&r).iter().all(|i| !i.vault_exists
            && !i.vault_biometric
            && i.vault_generation == 0
            && !i.vault_rotation_pending));
    }

    #[test]
    fn to_infos_with_maps_the_supplied_vault_flags_per_entry() {
        let r = registry();

        let infos = to_infos_with(
            &r,
            |c| ContextVaultInfo {
                exists: c.label == "Work",
                generation: if c.label == "Work" { 2 } else { 0 },
                rotation_pending: c.label == "Work",
                ..Default::default()
            },
            |c| c.label == "Personal",
        );

        let work = infos.iter().find(|i| i.label == "Work").unwrap();
        let personal = infos.iter().find(|i| i.label == "Personal").unwrap();
        assert!(work.vault_exists && !work.vault_biometric);
        assert!(work.vault_generation == 2 && work.vault_rotation_pending);
        assert!(!personal.vault_exists && personal.vault_biometric);
        assert!(personal.vault_generation == 0 && !personal.vault_rotation_pending);
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

    /// F8: the badge reads the pending rotation off the sync status instead
    /// of re-listing every context after every pull.
    #[test]
    fn the_status_carries_the_workspaces_pending_key_rotation() {
        let s = syncing_store();
        assert!(!sync_status_synced(&s).unwrap().vault_rotation_pending);
        crate::migrate::set_meta_i64(&s.conn, "vault_rotation_pending", 1).unwrap();
        assert!(sync_status_synced(&s).unwrap().vault_rotation_pending);

        // A local/unbound context has no workspace, so never a rotation.
        let r = Registry::default_for("/d.db");
        assert!(!sync_status_local(&r).unwrap().vault_rotation_pending);
    }

    /// R1: the guard the sync cycle uses to notice a context switch that
    /// happened while it was on the network.
    #[test]
    fn the_sync_epoch_invalidates_a_cycle_captured_before_a_swap() {
        let epoch = SyncEpoch::default();
        let captured = epoch.current();
        assert!(!epoch.changed_since(captured), "nothing swapped yet");

        epoch.bump(); // `swap_store_to` does exactly this
        assert!(epoch.changed_since(captured));

        // A cycle started after the swap is unaffected by the earlier one.
        let fresh = epoch.current();
        assert!(!epoch.changed_since(fresh));
        epoch.bump();
        assert!(epoch.changed_since(fresh) && epoch.changed_since(captured));
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
        encrypt_note_in_place(&s, "a", &Dek::random(), 1).unwrap();
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

        let pull = crate::sync::PullBody {
            cursor: 99,
            folders: vec![],
            notes: vec![],
            vault_keys: None,
            vault_generation: None,
            vault_rotation_pending: false,
            workspace_role: None,
            vault_invites: None,
        };
        commit_sync_result(&s, &push.note_ids, &push.folder_ids, &pull, 1_700).unwrap();

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

        let pull = crate::sync::PullBody {
            cursor: 5,
            folders: vec![],
            notes: vec![],
            vault_keys: None,
            vault_generation: None,
            vault_rotation_pending: false,
            workspace_role: None,
            vault_invites: None,
        };
        commit_sync_result(&s, &push.note_ids, &push.folder_ids, &pull, 1).unwrap();

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

        let pull = crate::sync::PullBody {
            cursor: 7,
            folders: vec![],
            notes: vec![pulled_note],
            vault_keys: None,
            vault_generation: None,
            vault_rotation_pending: false,
            workspace_role: None,
            vault_invites: None,
        };
        commit_sync_result(&s, &[], &[], &pull, 1).unwrap();

        let notes = s.load_notes().unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id, "server-note");
        assert_eq!(notes[0].content, "<p>from the server</p>");
        assert!(!notes[0].dirty, "a pulled row arrives clean");
    }

    #[test]
    fn apply_vault_keys_caches_entries_or_flags_a_legacy_server() {
        let s = Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        let with = crate::sync::PullBody {
            cursor: 1,
            folders: vec![],
            notes: vec![],
            vault_keys: Some(serde_json::json!({"mine": [], "recovery": []})),
            vault_generation: Some(1),
            vault_rotation_pending: false,
            workspace_role: None,
            vault_invites: None,
        };
        apply_vault_keys(&s, &with).unwrap();
        assert_eq!(
            s.vault_entries().unwrap().as_deref(),
            Some(r#"{"mine":[],"recovery":[]}"#)
        );
        assert_eq!(
            crate::migrate::get_meta_i64_opt(&s.conn, "vault_generation").unwrap(),
            Some(1)
        );
        assert_eq!(
            crate::migrate::get_meta_i64_opt(&s.conn, "vault_server_legacy").unwrap(),
            None
        );
        let legacy = crate::sync::PullBody {
            cursor: 2,
            folders: vec![],
            notes: vec![],
            vault_keys: None,
            vault_generation: None,
            vault_rotation_pending: false,
            workspace_role: None,
            vault_invites: None,
        };
        apply_vault_keys(&s, &legacy).unwrap();
        assert_eq!(
            crate::migrate::get_meta_i64_opt(&s.conn, "vault_server_legacy").unwrap(),
            Some(1)
        );
        assert_eq!(
            s.vault_entries().unwrap().as_deref(),
            Some(r#"{"mine":[],"recovery":[]}"#),
            "cache untouched"
        );
    }

    #[test]
    fn invites_needing_code_lists_missing_and_stale_wraps() {
        let json = r#"[{"invitationId":5,"generation":1},{"invitationId":6,"generation":null},{"invitationId":7,"generation":2}]"#;
        assert_eq!(invites_needing_code(json, 2), vec![5, 6]);
        assert_eq!(invites_needing_code(json, 1), vec![6]);
        assert!(invites_needing_code("not json", 2).is_empty());
        assert!(
            invites_needing_code(json, 0).is_empty(),
            "a vaultless context (generation 0) never counts invitations"
        );
    }

    #[test]
    fn recode_targets_and_marking_follow_the_cached_invites() {
        let s = Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        crate::migrate::set_meta_i64(&s.conn, "vault_generation", 2).unwrap();
        crate::migrate::set_meta(
            &s.conn,
            "vault_invites",
            r#"[{"invitationId":5,"generation":1},{"invitationId":6,"generation":2}]"#,
        )
        .unwrap();
        assert_eq!(recode_targets(&s).unwrap(), vec![5]);
        let updated = mark_invites_recoded(
            &crate::migrate::get_meta(&s.conn, "vault_invites")
                .unwrap()
                .unwrap(),
            &[5],
            2,
        );
        assert!(invites_needing_code(&updated, 2).is_empty());
        let fresh = Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&fresh.conn).unwrap();
        assert!(
            recode_targets(&fresh).unwrap().is_empty(),
            "no cached invites → nothing to re-code"
        );
    }

    #[test]
    fn apply_vault_keys_caches_the_role_and_the_open_invitations() {
        let s = Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        let pull = crate::sync::PullBody {
            cursor: 1,
            folders: vec![],
            notes: vec![],
            vault_keys: Some(serde_json::json!({"mine": [], "recovery": [], "rotation": []})),
            vault_generation: Some(2),
            vault_rotation_pending: false,
            workspace_role: Some("owner".into()),
            vault_invites: Some(serde_json::json!([{"invitationId": 5, "generation": null}])),
        };
        apply_vault_keys(&s, &pull).unwrap();
        assert_eq!(
            crate::migrate::get_meta(&s.conn, "workspace_role")
                .unwrap()
                .as_deref(),
            Some("owner")
        );
        assert_eq!(
            invites_needing_code(
                &crate::migrate::get_meta(&s.conn, "vault_invites")
                    .unwrap()
                    .unwrap(),
                2
            ),
            vec![5]
        );
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
        assert!(open_note_content(&s, &VaultState::default(), "n1").is_err());
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
        let mut vault = VaultState::default();
        vault.unlock(1, dek.clone());
        assert!(set_note_protected(&s, &vault, "n1", true).is_err());
        assert!(encrypt_note_in_place(&s, "n1", &dek, 1).is_err());
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
        let pull = crate::sync::PullBody {
            cursor: 1,
            folders: vec![],
            notes: vec![],
            vault_keys: None,
            vault_generation: None,
            vault_rotation_pending: false,
            workspace_role: None,
            vault_invites: None,
        };
        assert!(commit_sync_result(&s, &[("n1".into(), 1)], &[], &pull, 1).is_err());
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
        let mut vault = VaultState::default();
        vault.unlock(1, Dek::random());
        assert!(set_folder_locked(&s, &vault, "f", true).is_err());
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
        let mut vault = VaultState::default();
        vault.unlock(1, Dek::random());
        backfill_protected_titles(&s, &vault);
    }
}
