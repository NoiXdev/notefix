// src-tauri/src/sync.rs
//
// C1 sync engine. Pure mapping (ms-epoch <-> ISO8601, client model <-> server
// wire) + apply-pulled. Thin network calls + orchestration land in later tasks.

use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::folders::{self, Folder};
use crate::storage::{self, Note, Store};

/// Milliseconds since the Unix epoch -> RFC3339/ISO8601 (UTC), millisecond
/// precision preserved. Empty string on the (practically impossible) overflow.
pub fn ms_to_iso8601(ms: i64) -> String {
    OffsetDateTime::from_unix_timestamp_nanos((ms as i128) * 1_000_000)
        .ok()
        .and_then(|dt| dt.format(&Rfc3339).ok())
        .unwrap_or_default()
}

/// RFC3339/ISO8601 -> milliseconds since the Unix epoch. Accepts a `Z` suffix
/// or a numeric offset (Carbon emits `+00:00`). Returns 0 on parse failure.
pub fn iso8601_to_ms(s: &str) -> i64 {
    OffsetDateTime::parse(s, &Rfc3339)
        .ok()
        .map(|dt| (dt.unix_timestamp_nanos() / 1_000_000) as i64)
        .unwrap_or(0)
}

/// Maps a `Note` to the server wire JSON. `content` is pushed **verbatim** —
/// for a protected note this is already ciphertext (sealed by the vault
/// crypto layer before it ever reaches storage), and nothing in this module
/// decrypts it. `protected` travels alongside it so a receiving device knows
/// the bytes it just pulled are ciphertext and must not be rendered as
/// plaintext. Syncing the flag is safe on its own: it carries no secret, and
/// the content it describes stays unreadable without the vault DEK — which,
/// like the wrapped vault record and the biometric keychain entry, never
/// crosses the wire (a follow-up needs a dedicated vault-record sync entity
/// plus server support before a second device can unlock).
pub fn note_to_wire(n: &Note) -> Value {
    json!({
        "id": n.id,
        "folderId": n.folder_id,
        "content": n.content,
        "pinned": n.pinned,
        "archived": n.archived,
        "color": n.color,
        "dueAt": n.due_at.map(ms_to_iso8601),
        "position": n.position,
        "updatedAt": ms_to_iso8601(n.updated_at),
        "deletedAt": n.deleted_at.map(ms_to_iso8601),
        "protected": n.protected,
        // Plaintext metadata, synced like `folderId` — never encrypted, even
        // for a protected note. See `Note::title`.
        "title": n.title,
        // Which key generation sealed this note's ciphertext (see
        // `storage::Note::key_gen`) — null for an unprotected note or one
        // written before generations existed.
        "keyGen": n.key_gen,
    })
}

pub fn note_from_wire(v: &Value) -> Note {
    Note {
        id: v["id"].as_str().unwrap_or_default().to_string(),
        content: v["content"].as_str().unwrap_or_default().to_string(),
        updated_at: v["updatedAt"].as_str().map(iso8601_to_ms).unwrap_or(0),
        pinned: v["pinned"].as_bool().unwrap_or(false),
        archived: v["archived"].as_bool().unwrap_or(false),
        color: v["color"].as_str().unwrap_or_default().to_string(),
        due_at: v["dueAt"].as_str().map(iso8601_to_ms),
        folder_id: v["folderId"].as_str().map(str::to_string),
        position: v["position"].as_i64().unwrap_or(0),
        deleted_at: v["deletedAt"].as_str().map(iso8601_to_ms),
        dirty: false,
        // Missing/unknown-to-server field defaults to unprotected — matches
        // today's behavior for a server that doesn't persist this field yet.
        protected: v["protected"].as_bool().unwrap_or(false),
        // Did the wire payload actually carry `protected`? A server that
        // omits the field entirely must not be read as an explicit `false`
        // that would unprotect the note on pull — see
        // `upsert_note_from_server_conn`.
        protected_known: v["protected"].as_bool().is_some(),
        title: v["title"].as_str().unwrap_or_default().to_string(),
        // `mcp_hidden` is a LOCAL-only flag (see `storage::Note::mcp_hidden`)
        // and is never put on the wire in the first place — always false
        // here, and `upsert_note_from_server_conn` deliberately never writes
        // this column, so a pull can't clobber whatever was set locally.
        mcp_hidden: false,
        // Which key generation sealed this note's ciphertext (see
        // `storage::Note::key_gen`) — absent/null maps to `None`, matching an
        // unprotected note or one from a server that predates generations.
        key_gen: v["keyGen"].as_u64().map(|g| g as u32),
    }
}

pub fn folder_to_wire(f: &Folder) -> Value {
    json!({
        "id": f.id,
        "parentId": f.parent_id,
        "name": f.name,
        "icon": f.icon,
        "color": f.color,
        "sort": f.sort,
        "position": f.position,
        "updatedAt": ms_to_iso8601(f.updated_at),
        "deletedAt": f.deleted_at.map(ms_to_iso8601),
        "locked": f.locked,
    })
}

pub fn folder_from_wire(v: &Value) -> Folder {
    Folder {
        id: v["id"].as_str().unwrap_or_default().to_string(),
        name: v["name"].as_str().unwrap_or_default().to_string(),
        parent_id: v["parentId"].as_str().map(str::to_string),
        position: v["position"].as_i64().unwrap_or(0),
        icon: v["icon"].as_str().unwrap_or_default().to_string(),
        color: v["color"].as_str().unwrap_or_default().to_string(),
        sort: v["sort"].as_str().unwrap_or_default().to_string(),
        updated_at: v["updatedAt"].as_str().map(iso8601_to_ms).unwrap_or(0),
        deleted_at: v["deletedAt"].as_str().map(iso8601_to_ms),
        dirty: false,
        // `locked` now syncs too — same reasoning as `Note.protected` in
        // `note_to_wire` above: it only labels content that's already
        // ciphertext, so the flag alone carries no secret. Missing/unknown
        // field defaults to unlocked, matching today's behavior for a server
        // that doesn't persist this field yet.
        locked: v["locked"].as_bool().unwrap_or(false),
        // LOCAL-only flag, never on the wire — see `note_from_wire` above.
        mcp_hidden: false,
    }
}

/// Apply a pulled batch to the local cache (server rows win) in one transaction.
pub fn apply_pulled(store: &Store, folders: &[Value], notes: &[Value]) -> rusqlite::Result<()> {
    let tx = store.conn.unchecked_transaction()?;
    for fv in folders {
        folders::upsert_folder_from_server(&tx, &folder_from_wire(fv))?;
    }
    for nv in notes {
        storage::upsert_note_from_server_conn(&tx, &note_from_wire(nv))?;
    }
    tx.commit()
}

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceInfo {
    pub id: String,
    pub name: String,
    pub role: String,
}

/// Sync failure kinds. `Offline` is retryable (network/timeout/connection/401);
/// `Fatal` is a payload/server error that retrying won't fix. `Gone` is a
/// narrower `Fatal`: the server answered 410 for a resource that simply no
/// longer exists (e.g. an invitation that expired between the pull that
/// listed it and a later action on it) — not retryable either, but callers
/// that can sensibly skip just that one item rather than aborting the whole
/// operation match on it explicitly (see `vault_invite_recode`).
#[derive(Debug)]
pub enum SyncError {
    Offline(String),
    Fatal(String),
    Gone(String),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::Offline(m) | SyncError::Fatal(m) | SyncError::Gone(m) => write!(f, "{m}"),
        }
    }
}

/// A transport failure as an `Offline` error, WITHOUT the URL reqwest puts
/// into its `Display` ("error sending request for url (https://…/invite/<the
/// token>)"). One of these URLs carries an invitation token, another the
/// workspace id, and this string is surfaced to the user, logged by the sync
/// loop, and interpolated into UI copy.
///
/// reqwest exposes the URL as a field, so the sanitising is exact rather than
/// a search for the string: `without_url()` returns the same error with it
/// removed, and its `Display` then stops at the cause.
fn offline(e: reqwest::Error) -> SyncError {
    SyncError::Offline(e.without_url().to_string())
}

fn client() -> Result<reqwest::Client, SyncError> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| SyncError::Fatal(e.to_string()))
}

fn base(server_url: &str) -> String {
    server_url.trim_end_matches('/').to_string()
}

/// Classify an HTTP response status for our sync endpoints: 401 is treated as
/// `Offline` (retryable — token likely needs refreshing), any other non-2xx
/// is a `Fatal` error tagged with `context`, 2xx is `Ok`. Pulled out of
/// `fetch_workspaces`/`pull`/`push` so this classification is testable
/// without a network call.
fn classify_status(status: reqwest::StatusCode, context: &str) -> Result<(), SyncError> {
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(SyncError::Offline("unauthorized".into()));
    }
    if !status.is_success() {
        return Err(SyncError::Fatal(format!(
            "{context} HTTP {}",
            status.as_u16()
        )));
    }
    Ok(())
}

fn workspaces_url(server_url: &str) -> String {
    format!("{}/api/workspaces", base(server_url))
}

/// Map the `/api/workspaces` JSON body to `WorkspaceInfo`s. Pulled out of
/// [`fetch_workspaces`] so the response-parsing logic is testable without a
/// network call.
fn parse_workspaces(body: &Value) -> Vec<WorkspaceInfo> {
    body["data"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|w| WorkspaceInfo {
            id: w["id"].as_str().unwrap_or_default().to_string(),
            name: w["name"].as_str().unwrap_or_default().to_string(),
            role: w["role"].as_str().unwrap_or_default().to_string(),
        })
        .collect()
}

/// GET /api/workspaces — the user's workspaces for the picker.
pub async fn fetch_workspaces(
    server_url: &str,
    token: &str,
) -> Result<Vec<WorkspaceInfo>, SyncError> {
    let resp = client()?
        .get(workspaces_url(server_url))
        .bearer_auth(token)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(offline)?;
    classify_status(resp.status(), "workspaces")?;
    let body: Value = resp
        .json()
        .await
        .map_err(|e| SyncError::Fatal(e.to_string()))?;
    Ok(parse_workspaces(&body))
}

fn pull_url(server_url: &str, workspace_id: &str, since: i64) -> String {
    format!(
        "{}/api/workspaces/{}/changes?since={}",
        base(server_url),
        workspace_id,
        since
    )
}

/// A parsed `…/changes` GET response: the pulled rows plus whatever the
/// server said about the caller's workspace vault. `vault_keys` is `None`
/// when the server predates the vault-keys feature entirely (it simply omits
/// the field) — that's distinct from a present-but-empty `{"mine":[],
/// "recovery":[]}`, which means "no keys yet" on a server that does support
/// it.
#[derive(Debug, Clone)]
pub struct PullBody {
    pub cursor: i64,
    pub folders: Vec<Value>,
    pub notes: Vec<Value>,
    pub vault_keys: Option<Value>,
    pub vault_generation: Option<u32>,
    pub vault_rotation_pending: bool,
    /// The caller's role in the workspace, when the server reports it.
    pub workspace_role: Option<String>,
    /// For an owner: the open invitations with their wrap generation
    /// (`[{invitationId, generation|null}]`); absent on older servers.
    pub vault_invites: Option<Value>,
}

/// Map the `…/changes` GET JSON body to a [`PullBody`]. The server may nest
/// each collection under a `data` key (`{"folders": {"data": [...]}}`) or
/// send it as a bare array — both are accepted. Missing cursor falls back to
/// the `since` cursor the caller requested. Pulled out of [`pull`] so this
/// parsing logic is testable without a network call.
pub fn parse_pull_response(body: &Value, since: i64) -> PullBody {
    let arr = |k: &str| {
        body[k]["data"]
            .as_array()
            .or(body[k].as_array())
            .cloned()
            .unwrap_or_default()
    };
    PullBody {
        cursor: body["cursor"].as_i64().unwrap_or(since),
        folders: arr("folders"),
        notes: arr("notes"),
        vault_keys: body.get("vaultKeys").filter(|v| v.is_object()).cloned(),
        vault_generation: body["vaultGeneration"].as_u64().map(|g| g as u32),
        vault_rotation_pending: body["vaultRotationPending"].as_bool().unwrap_or(false),
        workspace_role: body["workspaceRole"].as_str().map(str::to_string),
        vault_invites: body.get("vaultInvites").filter(|v| v.is_array()).cloned(),
    }
}

/// GET …/changes?since= → the pulled rows plus vault-key state, as raw wire
/// values.
pub async fn pull(
    server_url: &str,
    token: &str,
    workspace_id: &str,
    since: i64,
) -> Result<PullBody, SyncError> {
    let resp = client()?
        .get(pull_url(server_url, workspace_id, since))
        .bearer_auth(token)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(offline)?;
    classify_status(resp.status(), "pull")?;
    let body: Value = resp
        .json()
        .await
        .map_err(|e| SyncError::Fatal(e.to_string()))?;
    Ok(parse_pull_response(&body, since))
}

fn push_url(server_url: &str, workspace_id: &str) -> String {
    format!(
        "{}/api/workspaces/{}/changes",
        base(server_url),
        workspace_id
    )
}

/// Map the `…/changes` POST JSON body to the server's new cursor (0 if
/// absent). Pulled out of [`push`] so this parsing logic is testable without
/// a network call.
fn parse_push_response(body: &Value) -> i64 {
    body["cursor"].as_i64().unwrap_or(0)
}

/// POST …/changes with dirty folders+notes; returns the server's new cursor.
pub async fn push(
    server_url: &str,
    token: &str,
    workspace_id: &str,
    folders: Vec<Value>,
    notes: Vec<Value>,
) -> Result<i64, SyncError> {
    let resp = client()?
        .post(push_url(server_url, workspace_id))
        .bearer_auth(token)
        .header("Accept", "application/json")
        .json(&json!({ "folders": folders, "notes": notes }))
        .send()
        .await
        .map_err(offline)?;
    classify_status(resp.status(), "push")?;
    let body: Value = resp
        .json()
        .await
        .map_err(|e| SyncError::Fatal(e.to_string()))?;
    Ok(parse_push_response(&body))
}

// ---------------------------------------------------------------------------
// Workspace vault keys
//
// The wrapped keys a server-bound context's vault lives on. Only wraps and
// the sealed DEK-check magic travel — never the DEK, never a passphrase.
// ---------------------------------------------------------------------------

fn vault_url(server_url: &str, ws: &str) -> String {
    format!("{}/api/workspaces/{ws}/vault", base(server_url))
}

fn vault_my_key_url(server_url: &str, ws: &str) -> String {
    format!("{}/api/workspaces/{ws}/vault/keys/me", base(server_url))
}

fn vault_invite_url(server_url: &str, ws: &str, invitation_id: u64) -> String {
    format!(
        "{}/api/workspaces/{ws}/vault/invites/{invitation_id}",
        base(server_url)
    )
}

fn vault_invite_accept_url(server_url: &str, ws: &str, invitation_id: u64) -> String {
    format!("{}/accept", vault_invite_url(server_url, ws, invitation_id))
}

fn vault_invite_by_token_url(server_url: &str, ws: &str, token: &str) -> String {
    format!(
        "{}/api/workspaces/{ws}/vault/invites/by-token/{token}",
        base(server_url)
    )
}

fn vault_rotate_url(server_url: &str, ws: &str) -> String {
    format!("{}/api/workspaces/{ws}/vault/rotate", base(server_url))
}

fn vault_recovery_url(server_url: &str, ws: &str) -> String {
    format!("{}/api/workspaces/{ws}/vault/recovery", base(server_url))
}

fn members_url(server_url: &str, ws: &str) -> String {
    format!("{}/api/workspaces/{ws}/members", base(server_url))
}

fn me_url(server_url: &str) -> String {
    format!("{}/api/user", base(server_url))
}

/// Whether `POST …/vault` seeded the workspace vault, or found one already
/// there. `AlreadyExists` is a normal outcome, not an error: two devices can
/// legitimately race to seed the same workspace.
pub enum VaultCreateOutcome {
    Created,
    AlreadyExists,
}

/// 409 → `AlreadyExists`; everything else goes through the usual
/// [`classify_status`] rules. Pulled out so the mapping is testable without
/// a network call.
fn classify_vault_create(status: reqwest::StatusCode) -> Result<VaultCreateOutcome, SyncError> {
    if status == reqwest::StatusCode::CONFLICT {
        return Ok(VaultCreateOutcome::AlreadyExists);
    }
    classify_status(status, "vault create")?;
    Ok(VaultCreateOutcome::Created)
}

/// POST …/vault — seed the workspace vault with generation 1's wrapped keys.
pub async fn vault_create(
    server_url: &str,
    token: &str,
    ws: &str,
    payload: &crate::ops::SetupPayload,
) -> Result<VaultCreateOutcome, SyncError> {
    let resp = client()?
        .post(vault_url(server_url, ws))
        .bearer_auth(token)
        .header("Accept", "application/json")
        .json(payload)
        .send()
        .await
        .map_err(offline)?;
    classify_vault_create(resp.status())
}

/// PUT …/vault/keys/me — replace the caller's own wrap for one generation
/// (the passphrase change path). The generation travels in the body.
pub async fn vault_put_my_key(
    server_url: &str,
    token: &str,
    ws: &str,
    entry: &crate::ops::MyEntryWire,
) -> Result<(), SyncError> {
    let resp = client()?
        .put(vault_my_key_url(server_url, ws))
        .bearer_auth(token)
        .header("Accept", "application/json")
        .json(entry)
        .send()
        .await
        .map_err(offline)?;
    classify_status(resp.status(), "vault key")
}

/// 410 becomes `SyncError::Gone`; everything else goes through the usual
/// [`classify_status`] rules. Pulled out so the mapping is testable without a
/// network call — mirrors [`classify_vault_create`]/[`classify_vault_recovery`].
///
/// The invitation can expire between the pull that listed it as open (or lost
/// its wrap in a rotation) and this attach — the server then answers 410
/// rather than 404, since the invitation id itself is well-formed and simply
/// no longer accepts a wrap. `vault_invite_recode` reads that as "skip this
/// one, the next pull will drop it from the cache" rather than aborting the
/// whole re-code loop.
fn classify_vault_attach(status: reqwest::StatusCode) -> Result<(), SyncError> {
    if status == reqwest::StatusCode::GONE {
        return Err(SyncError::Gone("vault invite HTTP 410".to_string()));
    }
    classify_status(status, "vault invite")
}

/// POST …/vault/invites/{id} — attach the newest DEK, wrapped under a
/// one-time invite code, to an invitation the owner just created.
pub async fn vault_attach_invite(
    server_url: &str,
    token: &str,
    ws: &str,
    invitation_id: u64,
    wrap: &crate::ops::InviteWrap,
) -> Result<(), SyncError> {
    let resp = client()?
        .post(vault_invite_url(server_url, ws, invitation_id))
        .bearer_auth(token)
        .header("Accept", "application/json")
        .json(wrap)
        .send()
        .await
        .map_err(offline)?;
    classify_vault_attach(resp.status())
}

/// GET …/vault/invites/{id} — the invite wrap waiting for this caller. Only
/// the invitee who accepted the invitation can fetch it; everyone else gets a
/// 404, which arrives here as a `Fatal`.
pub async fn vault_fetch_invite(
    server_url: &str,
    token: &str,
    ws: &str,
    invitation_id: u64,
) -> Result<crate::ops::InviteWrap, SyncError> {
    let resp = client()?
        .get(vault_invite_url(server_url, ws, invitation_id))
        .bearer_auth(token)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(offline)?;
    classify_status(resp.status(), "vault invite")?;
    resp.json()
        .await
        .map_err(|e| SyncError::Fatal(e.to_string()))
}

/// POST …/vault/invites/{id}/accept — swap the invite wrap for the member's
/// own passphrase wrap. The server deletes the invite wrap in the same
/// transaction, so the one-time code is spent.
pub async fn vault_accept_invite(
    server_url: &str,
    token: &str,
    ws: &str,
    invitation_id: u64,
    entry: &crate::ops::MyEntryWire,
) -> Result<(), SyncError> {
    let resp = client()?
        .post(vault_invite_accept_url(server_url, ws, invitation_id))
        .bearer_auth(token)
        .header("Accept", "application/json")
        .json(entry)
        .send()
        .await
        .map_err(offline)?;
    classify_status(resp.status(), "vault invite accept")
}

/// GET …/vault/invites/by-token/{token} — the numeric id behind a share link.
/// Users only ever see the link, but every other invite endpoint is keyed by
/// the id.
pub async fn vault_resolve_invite(
    server_url: &str,
    token: &str,
    ws: &str,
    invite_token: &str,
) -> Result<u64, SyncError> {
    let resp = client()?
        .get(vault_invite_by_token_url(server_url, ws, invite_token))
        .bearer_auth(token)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(offline)?;
    classify_status(resp.status(), "vault invite lookup")?;
    let body: Value = resp
        .json()
        .await
        .map_err(|e| SyncError::Fatal(e.to_string()))?;
    parse_invitation_id(&body)
}

/// The `{"invitationId": n}` body of the lookup above. Pulled out so the
/// parsing is testable without a network call.
fn parse_invitation_id(body: &Value) -> Result<u64, SyncError> {
    body["invitationId"]
        .as_u64()
        .ok_or_else(|| SyncError::Fatal("vault invite lookup: no invitation id".into()))
}

/// One row of the members listing. `name` is the display name the server
/// knows (empty when the account has none) — used to label rotation codes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberInfo {
    pub user_id: u64,
    pub role: String,
    pub name: String,
}

/// The `{"members": [{"userId": n, "role": "...", "name": "..."}]}` body of
/// the members listing. Pulled out so the parsing is testable without a
/// network call.
fn parse_members_response(body: &Value) -> Vec<MemberInfo> {
    body["members"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|m| {
            Some(MemberInfo {
                user_id: m["userId"].as_u64()?,
                role: m["role"].as_str().unwrap_or_default().to_string(),
                name: m["name"].as_str().unwrap_or_default().to_string(),
            })
        })
        .collect()
}

/// GET /api/user — the authenticated user's own id, needed to tell the
/// caller's own rotation entry apart from everyone else's. Nothing else from
/// that response is read, kept or logged.
pub async fn fetch_me(server_url: &str, token: &str) -> Result<u64, SyncError> {
    let resp = client()?
        .get(me_url(server_url))
        .bearer_auth(token)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(offline)?;
    classify_status(resp.status(), "user")?;
    let body: Value = resp
        .json()
        .await
        .map_err(|e| SyncError::Fatal(e.to_string()))?;
    body["id"]
        .as_u64()
        .ok_or_else(|| SyncError::Fatal("user: no id".into()))
}

/// GET …/members — owner plus members of the workspace.
pub async fn fetch_members(
    server_url: &str,
    token: &str,
    ws: &str,
) -> Result<Vec<MemberInfo>, SyncError> {
    let resp = client()?
        .get(members_url(server_url, ws))
        .bearer_auth(token)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(offline)?;
    classify_status(resp.status(), "members")?;
    let body: Value = resp
        .json()
        .await
        .map_err(|e| SyncError::Fatal(e.to_string()))?;
    Ok(parse_members_response(&body))
}

/// POST …/vault/rotate — install a new key generation: one wrap per remaining
/// member (the caller's own under their passphrase, everyone else's under a
/// one-time rotation code) plus, when the caller holds it, the recovery wrap.
/// Returns the generation the workspace ended up on.
pub async fn vault_rotate(
    server_url: &str,
    token: &str,
    ws: &str,
    payload: &crate::ops::RotatePayload,
) -> Result<u32, SyncError> {
    let resp = client()?
        .post(vault_rotate_url(server_url, ws))
        .bearer_auth(token)
        .header("Accept", "application/json")
        .json(payload)
        .send()
        .await
        .map_err(offline)?;
    classify_status(resp.status(), "vault rotate")?;
    let body: Value = resp
        .json()
        .await
        .map_err(|e| SyncError::Fatal(e.to_string()))?;
    Ok(parse_vault_generation(&body).unwrap_or(payload.generation))
}

/// The `{"vaultGeneration": n}` body of a rotation. Falls back to the
/// generation the caller asked for when the server answers without one.
fn parse_vault_generation(body: &Value) -> Option<u32> {
    body["vaultGeneration"].as_u64().map(|g| g as u32)
}

/// How a 409 on `POST …/vault/recovery` should be read. The creator's
/// follow-up resubmits the wrap it just derived from the vault's EXISTING
/// recovery key for a generation somebody else rotated — a 409 there means
/// the wrap is already there under that same key, exactly the state the
/// caller wanted, so `AlreadyDone` reports it as `Ok`. Minting a BRAND NEW
/// key (`vault_recovery_create`) is different: a 409 there means the
/// workspace already holds a wrap for (workspace, generation, this user)
/// under a DIFFERENT key, and reporting that as success would hand the user
/// a key that opens nothing for that generation — `Reject` turns it into an
/// error instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryConflict {
    AlreadyDone,
    Reject,
}

/// 409 branches on `on_conflict` (see [`RecoveryConflict`]); everything else
/// goes through the usual [`classify_status`] rules. Pulled out so the
/// mapping is testable without a network call — mirrors
/// [`classify_vault_create`].
fn classify_vault_recovery(
    status: reqwest::StatusCode,
    on_conflict: RecoveryConflict,
) -> Result<(), SyncError> {
    if status == reqwest::StatusCode::CONFLICT {
        return match on_conflict {
            RecoveryConflict::AlreadyDone => Ok(()),
            RecoveryConflict::Reject => Err(SyncError::Fatal(
                "vault: the workspace already holds a recovery key from you — sync first"
                    .to_string(),
            )),
        };
    }
    classify_status(status, "vault recovery")
}

/// POST …/vault/recovery — attach one generation's recovery wrap.
/// `on_conflict` says how to read a 409; see [`RecoveryConflict`].
pub async fn vault_post_recovery(
    server_url: &str,
    token: &str,
    ws: &str,
    generation: u32,
    payload: &crate::ops::RecoveryPayload,
    on_conflict: RecoveryConflict,
) -> Result<(), SyncError> {
    let body = serde_json::json!({
        "generation": generation,
        "recoverySalt": payload.recovery_salt,
        "dekWrappedRecovery": payload.dek_wrapped_recovery,
        "dekCheck": payload.dek_check,
    });
    let resp = client()?
        .post(vault_recovery_url(server_url, ws))
        .bearer_auth(token)
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(offline)?;
    classify_vault_recovery(resp.status(), on_conflict)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal `Note` for wire-mapping tests — mirrors `ops::test_support::note`.
    fn note(id: &str, content: &str) -> Note {
        Note {
            id: id.into(),
            content: content.into(),
            updated_at: 1,
            ..Default::default()
        }
    }

    #[test]
    fn iso_ms_roundtrip_preserves_milliseconds() {
        let ms = 1_700_000_000_123; // not second-aligned: proves ms precision
        assert_eq!(iso8601_to_ms(&ms_to_iso8601(ms)), ms);
    }

    #[test]
    fn parses_carbon_style_numeric_offset() {
        // Carbon's toIso8601String() emits a +00:00 offset, not Z.
        assert_eq!(
            iso8601_to_ms("2023-11-14T22:13:20+00:00"),
            1_700_000_000_000
        );
    }

    #[test]
    fn note_wire_roundtrip() {
        let n = Note {
            id: "n1".into(),
            content: "<p>x</p>".into(),
            updated_at: 1_700_000_000_000,
            pinned: true,
            archived: false,
            color: "red".into(),
            due_at: Some(1_700_000_001_000),
            folder_id: Some("f1".into()),
            position: 3,
            deleted_at: None,
            dirty: true,
            protected: false,
            protected_known: false,
            title: "My Title".into(),
            mcp_hidden: false,
            key_gen: None,
        };
        let back = note_from_wire(&note_to_wire(&n));
        assert_eq!(back.id, "n1");
        assert_eq!(back.updated_at, n.updated_at);
        assert_eq!(back.due_at, n.due_at);
        assert_eq!(back.folder_id, n.folder_id);
        assert!(!back.dirty); // wire never carries dirty
        assert!(!back.protected);
        assert_eq!(back.title, n.title);
    }

    #[test]
    fn note_wire_never_carries_mcp_hidden() {
        // "Hide from MCP" (schema v14) is a LOCAL-only opt-out — it must never
        // reach the wire payload, protected or not.
        let n = Note {
            id: "n1".into(),
            mcp_hidden: true,
            ..Default::default()
        };
        let wire = note_to_wire(&n);
        assert!(
            wire.get("mcpHidden").is_none() && wire.get("mcp_hidden").is_none(),
            "got: {wire}"
        );
        // And a server round-trip can't turn it on either.
        assert!(!note_from_wire(&wire).mcp_hidden);
    }

    #[test]
    fn note_from_wire_defaults_title_empty_when_field_missing() {
        // A server that doesn't (yet) persist `title` simply omits it.
        let v = json!({ "id": "n1", "content": "<p>x</p>" });
        assert_eq!(note_from_wire(&v).title, "");
    }

    #[test]
    fn note_wire_roundtrip_preserves_protected_ciphertext() {
        // Opaque, base64-like blob standing in for real vault ciphertext — the
        // point of this test is that sync never inspects or decrypts it.
        let ciphertext = "v1:Zm9vYmFyYmF6cXV1eA==:9f8a7b6c5d4e3f2a1b0c";
        let n = Note {
            id: "n1".into(),
            content: ciphertext.into(),
            updated_at: 1_700_000_000_000,
            protected: true,
            ..Default::default()
        };

        let wire = note_to_wire(&n);
        // The wire payload carries the ciphertext byte-for-byte, plus the flag
        // that tells the receiver it IS ciphertext — never plaintext.
        assert_eq!(wire["content"], ciphertext);
        assert_eq!(wire["protected"], true);

        let back = note_from_wire(&wire);
        assert!(back.protected);
        assert_eq!(back.content, ciphertext);
    }

    #[test]
    fn note_from_wire_defaults_protected_false_when_field_missing() {
        // A server that doesn't (yet) persist `protected` simply omits it —
        // must not be mistaken for an explicit `false` that unprotects a note.
        let v = json!({ "id": "n1", "content": "<p>x</p>" });
        assert!(!note_from_wire(&v).protected);
    }

    #[test]
    fn note_from_wire_marks_protected_unknown_when_field_missing() {
        let n = note_from_wire(
            &json!({"id":"a","content":"cipher==","updatedAt":"2026-01-01T00:00:00Z"}),
        );
        assert!(!n.protected_known);
        let m = note_from_wire(
            &json!({"id":"a","content":"<p>x</p>","protected":false,"updatedAt":"2026-01-01T00:00:00Z"}),
        );
        assert!(m.protected_known && !m.protected);
    }

    #[test]
    fn note_wire_carries_key_gen() {
        let mut n = note("a", "cipher==");
        n.protected = true;
        n.key_gen = Some(3);
        let w = note_to_wire(&n);
        assert_eq!(w["keyGen"], 3);
        assert_eq!(note_from_wire(&w).key_gen, Some(3));
        assert_eq!(
            note_from_wire(&json!({"id":"a","content":"x","updatedAt":"2026-01-01T00:00:00Z"}))
                .key_gen,
            None
        );
    }

    #[test]
    fn folder_wire_roundtrip_and_tombstone() {
        let f = Folder {
            id: "f1".into(),
            name: "Work".into(),
            parent_id: None,
            position: 2,
            icon: "star".into(),
            color: "blue".into(),
            sort: "manual".into(),
            updated_at: 1_700_000_000_000,
            deleted_at: Some(1_700_000_005_000),
            dirty: true,
            locked: false,
            mcp_hidden: false,
        };
        let back = folder_from_wire(&folder_to_wire(&f));
        assert_eq!(back.name, "Work");
        assert_eq!(back.deleted_at, f.deleted_at);
        assert!(!back.locked);
    }

    #[test]
    fn folder_wire_roundtrip_preserves_locked() {
        let f = Folder {
            id: "f1".into(),
            name: "Secrets".into(),
            locked: true,
            ..Default::default()
        };
        let wire = folder_to_wire(&f);
        assert_eq!(wire["locked"], true);
        let back = folder_from_wire(&wire);
        assert!(back.locked);
    }

    #[test]
    fn apply_pulled_overwrites_local() {
        let s = Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        s.save_note(&Note {
            id: "n1".into(),
            content: "local".into(),
            updated_at: 1,
            ..Default::default()
        })
        .unwrap();
        let server_note = note_to_wire(&Note {
            id: "n1".into(),
            content: "server".into(),
            updated_at: 1_700_000_000_000,
            ..Default::default()
        });
        apply_pulled(&s, &[], &[server_note]).unwrap();
        assert_eq!(s.load_all_notes().unwrap()[0].content, "server");
    }

    #[test]
    fn apply_pulled_never_clobbers_local_mcp_hidden() {
        // `mcp_hidden` is LOCAL-only: a pulled note/folder update (which never
        // carries the field — see `note_wire_never_carries_mcp_hidden`) must
        // not reset a locally-set flag back to false.
        let s = Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();
        s.save_note(&Note {
            id: "n1".into(),
            content: "local".into(),
            updated_at: 1,
            ..Default::default()
        })
        .unwrap();
        s.set_note_mcp_hidden("n1", true).unwrap();
        crate::folders::create_folder(&s.conn, "f1", "Work", None).unwrap();
        s.set_folder_mcp_hidden("f1", true).unwrap();

        let server_note = note_to_wire(&Note {
            id: "n1".into(),
            content: "server".into(),
            updated_at: 1_700_000_000_000,
            ..Default::default()
        });
        let server_folder = folder_to_wire(&Folder {
            id: "f1".into(),
            name: "Work".into(),
            updated_at: 1_700_000_000_000,
            ..Default::default()
        });
        apply_pulled(&s, &[server_folder], &[server_note]).unwrap();

        assert_eq!(s.load_all_notes().unwrap()[0].content, "server"); // other fields DO update
        assert!(s.note_mcp_hidden("n1").unwrap(), "local flag must survive");
        assert!(
            s.folder_mcp_hidden("f1").unwrap(),
            "local flag must survive"
        );
    }

    #[test]
    fn apply_pulled_persists_protected_and_locked_to_db() {
        // The wire-mapping tests above only check note_to_wire/note_from_wire
        // in isolation. This exercises the real pull-apply path
        // (apply_pulled -> upsert_note_from_server_conn /
        // upsert_folder_from_server) to confirm the flags actually land in
        // the SQLite columns, not just the in-memory struct.
        let s = Store::open_in_memory().unwrap();
        crate::migrate::run_migrations(&s.conn).unwrap();

        let ciphertext = "v1:c2VhbGVkLWJ5dGVz:deadbeef";
        let server_note = note_to_wire(&Note {
            id: "n1".into(),
            content: ciphertext.into(),
            updated_at: 1_700_000_000_000,
            protected: true,
            ..Default::default()
        });
        let server_folder = folder_to_wire(&Folder {
            id: "f1".into(),
            name: "Secrets".into(),
            updated_at: 1_700_000_000_000,
            locked: true,
            ..Default::default()
        });

        apply_pulled(&s, &[server_folder], &[server_note]).unwrap();

        let notes = s.load_all_notes().unwrap();
        assert_eq!(notes.len(), 1);
        assert!(notes[0].protected, "protected flag must persist to the DB");
        // Content stays exactly as received — never decrypted on the way in.
        assert_eq!(notes[0].content, ciphertext);

        let folders = folders::load_folders(&s.conn).unwrap();
        assert_eq!(folders.len(), 1);
        assert!(folders[0].locked, "locked flag must persist to the DB");
    }

    #[test]
    fn sync_error_display() {
        assert_eq!(SyncError::Offline("x".into()).to_string(), "x");
        assert_eq!(SyncError::Fatal("y".into()).to_string(), "y");
    }

    #[test]
    fn base_trims_trailing_slash() {
        assert_eq!(base("https://sync.test/"), "https://sync.test");
        assert_eq!(base("https://sync.test"), "https://sync.test");
    }

    /// A transport failure must never surface the URL it was for: the invite
    /// endpoints carry a one-time token in their path, and this string
    /// reaches the UI and the sync log.
    #[tokio::test]
    async fn a_transport_failure_never_carries_the_url_it_was_for() {
        // Port 1 on loopback refuses immediately — no network, no timeout.
        let err = vault_resolve_invite("http://127.0.0.1:1", "tok", "ws-1", "SECRET-INVITE-TOKEN")
            .await
            .expect_err("a refused connection must fail");
        let msg = err.to_string();
        assert!(matches!(err, SyncError::Offline(_)), "retryable: {msg}");
        assert!(
            !msg.contains("SECRET-INVITE-TOKEN"),
            "leaked the token: {msg}"
        );
        assert!(!msg.contains("127.0.0.1"), "leaked the URL: {msg}");
    }

    #[test]
    fn classify_status_unauthorized_is_offline() {
        assert!(matches!(
            classify_status(reqwest::StatusCode::UNAUTHORIZED, "pull"),
            Err(SyncError::Offline(m)) if m == "unauthorized"
        ));
    }

    #[test]
    fn classify_status_server_error_is_fatal_with_context() {
        let err = classify_status(reqwest::StatusCode::INTERNAL_SERVER_ERROR, "push")
            .expect_err("500 must fail");
        assert!(matches!(err, SyncError::Fatal(m) if m == "push HTTP 500"));
    }

    #[test]
    fn classify_status_success_is_ok() {
        assert!(classify_status(reqwest::StatusCode::OK, "pull").is_ok());
    }

    #[test]
    fn workspaces_pull_and_push_urls_are_well_formed() {
        assert_eq!(
            workspaces_url("https://sync.test/"),
            "https://sync.test/api/workspaces"
        );
        assert_eq!(
            pull_url("https://sync.test", "w1", 42),
            "https://sync.test/api/workspaces/w1/changes?since=42"
        );
        assert_eq!(
            push_url("https://sync.test", "w1"),
            "https://sync.test/api/workspaces/w1/changes"
        );
    }

    #[test]
    fn parse_workspaces_maps_fields() {
        let body = json!({"data": [
            {"id": "w1", "name": "Team", "role": "owner"},
            {"id": "w2", "name": "Solo", "role": "member"},
        ]});
        let ws = parse_workspaces(&body);
        assert_eq!(ws.len(), 2);
        assert_eq!(ws[0].id, "w1");
        assert_eq!(ws[0].name, "Team");
        assert_eq!(ws[0].role, "owner");
        assert_eq!(ws[1].id, "w2");
    }

    #[test]
    fn parse_workspaces_empty_when_no_data_field() {
        assert!(parse_workspaces(&json!({})).is_empty());
    }

    #[test]
    fn parse_pull_response_reads_nested_data_envelope() {
        let body = json!({
            "cursor": 100,
            "folders": {"data": [{"id": "f1"}]},
            "notes": {"data": [{"id": "n1"}, {"id": "n2"}]},
        });
        let p = parse_pull_response(&body, 0);
        assert_eq!(p.cursor, 100);
        assert_eq!(p.folders.len(), 1);
        assert_eq!(p.notes.len(), 2);
    }

    #[test]
    fn parse_pull_response_accepts_bare_arrays_and_defaults_cursor() {
        let body = json!({"folders": [{"id": "f1"}], "notes": []});
        let p = parse_pull_response(&body, 7);
        assert_eq!(p.cursor, 7, "missing cursor falls back to `since`");
        assert_eq!(p.folders.len(), 1);
        assert!(p.notes.is_empty());
    }

    #[test]
    fn parse_pull_response_reads_vault_keys_and_generation() {
        let body = json!({"cursor": 7, "folders": [], "notes": [],
            "vaultGeneration": 2, "vaultRotationPending": true,
            "vaultKeys": {"mine": [{"generation": 1, "kdfParams": {}, "dekWrapped": "w", "dekCheck": "c"}], "recovery": []}});
        let p = parse_pull_response(&body, 0);
        assert_eq!(p.cursor, 7);
        assert_eq!(p.vault_generation, Some(2));
        assert!(p.vault_rotation_pending);
        assert_eq!(p.vault_keys.as_ref().unwrap()["mine"][0]["dekWrapped"], "w");
        let legacy = parse_pull_response(&json!({"cursor": 1, "folders": [], "notes": []}), 0);
        assert!(legacy.vault_keys.is_none() && legacy.vault_generation.is_none());

        let body = json!({"cursor": 7, "folders": [], "notes": [], "vaultGeneration": 2, "vaultRotationPending": true,
            "vaultKeys": {"mine": [], "recovery": [], "rotation": []},
            "workspaceRole": "owner", "vaultInvites": [{"invitationId": 5, "generation": 1}, {"invitationId": 6, "generation": null}]});
        let p = parse_pull_response(&body, 0);
        assert_eq!(p.workspace_role.as_deref(), Some("owner"));
        assert_eq!(
            p.vault_invites.as_ref().unwrap()[1]["generation"],
            serde_json::Value::Null
        );
        let legacy = parse_pull_response(&json!({"cursor": 1, "folders": [], "notes": []}), 0);
        assert!(legacy.workspace_role.is_none() && legacy.vault_invites.is_none());
    }

    #[test]
    fn vault_endpoint_urls_are_well_formed() {
        assert_eq!(
            vault_url("https://s/", "ws1"),
            "https://s/api/workspaces/ws1/vault"
        );
        assert_eq!(
            vault_my_key_url("https://s", "ws1"),
            "https://s/api/workspaces/ws1/vault/keys/me"
        );
        assert_eq!(
            vault_invite_url("https://s/", "ws1", 7),
            "https://s/api/workspaces/ws1/vault/invites/7"
        );
        assert_eq!(
            vault_invite_accept_url("https://s", "ws1", 7),
            "https://s/api/workspaces/ws1/vault/invites/7/accept"
        );
        assert_eq!(
            vault_invite_by_token_url("https://s", "ws1", "tok"),
            "https://s/api/workspaces/ws1/vault/invites/by-token/tok"
        );
        assert_eq!(
            vault_rotate_url("https://s/", "ws1"),
            "https://s/api/workspaces/ws1/vault/rotate"
        );
        assert_eq!(
            vault_recovery_url("https://s", "ws1"),
            "https://s/api/workspaces/ws1/vault/recovery"
        );
        assert_eq!(
            members_url("https://s/", "ws1"),
            "https://s/api/workspaces/ws1/members"
        );
        assert_eq!(me_url("https://s/"), "https://s/api/user");
    }

    #[test]
    fn parse_members_response_reads_ids_roles_and_names() {
        let body = json!({"members": [
            {"userId": 1, "role": "owner", "name": "Anna"},
            {"userId": 2, "role": "editor"},
            {"role": "editor"},
        ]});
        assert_eq!(
            parse_members_response(&body),
            vec![
                MemberInfo {
                    user_id: 1,
                    role: "owner".into(),
                    name: "Anna".into()
                },
                MemberInfo {
                    user_id: 2,
                    role: "editor".into(),
                    name: String::new()
                },
            ]
        );
        assert!(parse_members_response(&json!({})).is_empty());
    }

    #[test]
    fn parse_vault_generation_reads_the_new_generation() {
        assert_eq!(
            parse_vault_generation(&json!({"vaultGeneration": 3})),
            Some(3)
        );
        assert_eq!(parse_vault_generation(&json!({})), None);
    }

    #[test]
    fn parse_invitation_id_reads_the_id_and_rejects_a_body_without_one() {
        assert_eq!(parse_invitation_id(&json!({"invitationId": 7})).unwrap(), 7);
        assert!(matches!(
            parse_invitation_id(&json!({})),
            Err(SyncError::Fatal(m)) if m == "vault invite lookup: no invitation id"
        ));
    }

    #[test]
    fn vault_create_outcome_maps_409_to_already_exists() {
        assert!(matches!(
            classify_vault_create(reqwest::StatusCode::CREATED).unwrap(),
            VaultCreateOutcome::Created
        ));
        assert!(matches!(
            classify_vault_create(reqwest::StatusCode::CONFLICT).unwrap(),
            VaultCreateOutcome::AlreadyExists
        ));
        assert!(classify_vault_create(reqwest::StatusCode::FORBIDDEN).is_err());
    }

    #[test]
    fn vault_attach_maps_410_to_gone_and_leaves_everything_else_alone() {
        assert!(matches!(
            classify_vault_attach(reqwest::StatusCode::CREATED),
            Ok(())
        ));
        assert!(matches!(
            classify_vault_attach(reqwest::StatusCode::GONE),
            Err(SyncError::Gone(m)) if m == "vault invite HTTP 410"
        ));
        assert!(matches!(
            classify_vault_attach(reqwest::StatusCode::FORBIDDEN),
            Err(SyncError::Fatal(_))
        ));
        assert!(matches!(
            classify_vault_attach(reqwest::StatusCode::UNAUTHORIZED),
            Err(SyncError::Offline(_))
        ));
    }

    #[test]
    fn vault_recovery_conflict_reads_by_mode() {
        // The follow-up resubmits the SAME key: 409 is the state it wanted.
        assert!(matches!(
            classify_vault_recovery(reqwest::StatusCode::CREATED, RecoveryConflict::AlreadyDone),
            Ok(())
        ));
        assert!(matches!(
            classify_vault_recovery(reqwest::StatusCode::CONFLICT, RecoveryConflict::AlreadyDone),
            Ok(())
        ));
        // A brand new key: 409 means a DIFFERENT wrap already sits there.
        assert!(matches!(
            classify_vault_recovery(reqwest::StatusCode::CONFLICT, RecoveryConflict::Reject),
            Err(SyncError::Fatal(m)) if m == "vault: the workspace already holds a recovery key from you — sync first"
        ));
        assert!(
            classify_vault_recovery(reqwest::StatusCode::FORBIDDEN, RecoveryConflict::Reject)
                .is_err()
        );
    }

    #[test]
    fn parse_push_response_reads_cursor_or_defaults_to_zero() {
        assert_eq!(parse_push_response(&json!({"cursor": 55})), 55);
        assert_eq!(parse_push_response(&json!({})), 0);
    }
}
