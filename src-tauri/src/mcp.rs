use crate::folders::Folder;
use crate::storage::Note;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager};

// Several methods below (`save`, `set_folder`, `set_archived`, `trash`,
// `untrash`, `list_folders`, `create_folder`, `now_ms`, `new_id`,
// `emit_changed`) have no non-test caller yet — the read/write tool logic
// that wires them up lands in Tasks 7-11. Remove this allow once they do.
#[allow(dead_code)]
pub trait NoteStore: Send + Sync {
    /// All notes incl. archived and trashed (like `load_all_notes`).
    fn all_notes(&self) -> Result<Vec<Note>, String>;
    fn get_note(&self, id: &str) -> Result<Option<Note>, String>;
    /// Upsert. On an existing row only content/updated_at change (see Global
    /// Constraints) — used for create (insert), append, and content replace.
    fn save(&self, note: &Note) -> Result<(), String>;
    fn set_folder(&self, id: &str, folder_id: Option<&str>) -> Result<(), String>;
    fn set_archived(&self, id: &str, archived: bool) -> Result<(), String>;
    fn trash(&self, id: &str, ts: i64) -> Result<(), String>;
    /// Clear the trash tombstone (does not touch `archived`).
    fn untrash(&self, id: &str) -> Result<(), String>;
    fn list_folders(&self) -> Result<Vec<Folder>, String>;
    /// Create a folder with a fresh id; returns the created `Folder`.
    fn create_folder(&self, name: &str, parent_id: Option<&str>) -> Result<Folder, String>;
    fn now_ms(&self) -> i64;
    fn new_id(&self) -> String;
    fn emit_changed(&self);
}

#[allow(dead_code)]
pub fn html_to_text(html: &str) -> String {
    let nl = regex::Regex::new(r"(?is)</(p|div|h[1-6]|li)>|<br\s*/?>")
        .unwrap()
        .replace_all(html, "\n");
    let stripped = regex::Regex::new(r"(?is)<[^>]+>")
        .unwrap()
        .replace_all(&nl, "");
    stripped
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .trim()
        .to_string()
}

#[allow(dead_code)]
pub fn text_to_html(text: &str) -> String {
    if text.is_empty() {
        return "<p></p>".to_string();
    }
    text.lines()
        .map(|l| {
            format!(
                "<p>{}</p>",
                l.replace('&', "&amp;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;")
            )
        })
        .collect()
}

fn ok(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}
fn rpc_err(id: &Value, code: i64, msg: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": msg } })
}

fn tool_defs() -> Value {
    json!([
        { "name": "list_notes", "description": "List all notes (id and title).", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "get_note", "description": "Get a note's text by id.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } },
        { "name": "search_notes", "description": "Search notes by text (case-insensitive).", "inputSchema": { "type": "object", "properties": { "query": { "type": "string" } }, "required": ["query"] } },
        { "name": "create_note", "description": "Create a new note from text.", "inputSchema": { "type": "object", "properties": { "content": { "type": "string" } }, "required": ["content"] } },
        { "name": "append_note", "description": "Append text to a note.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" }, "text": { "type": "string" } }, "required": ["id", "text"] } }
    ])
}

fn call_tool(
    name: &str,
    args: &Value,
    store: &dyn NoteStore,
    allow_write: bool,
) -> Result<String, String> {
    let _ = (args, allow_write);
    match name {
        "list_notes" => {
            let notes = store.all_notes()?;
            Ok(json!(notes
                .iter()
                .map(|n| json!({"id": n.id.clone()}))
                .collect::<Vec<_>>())
            .to_string())
        }
        _ => Err(format!("unknown tool {name}")),
    }
}

/// Returns None for notifications (no response).
pub fn handle_rpc(
    req: &Value,
    store: &dyn NoteStore,
    allow_write: bool,
    version: &str,
) -> Option<Value> {
    let id = req.get("id").cloned().unwrap_or(Value::Null);
    match req.get("method").and_then(|m| m.as_str()).unwrap_or("") {
        "initialize" => Some(ok(
            &id,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {}, "resources": {} },
                "serverInfo": { "name": "Notefix", "version": version }
            }),
        )),
        "notifications/initialized" => None,
        "ping" => Some(ok(&id, json!({}))),
        "tools/list" => Some(ok(&id, json!({ "tools": tool_defs() }))),
        "tools/call" => {
            let name = req
                .pointer("/params/name")
                .and_then(|n| n.as_str())
                .unwrap_or("");
            let args = req
                .pointer("/params/arguments")
                .cloned()
                .unwrap_or(json!({}));
            Some(match call_tool(name, &args, store, allow_write) {
                Ok(text) => ok(
                    &id,
                    json!({ "content": [{ "type": "text", "text": text }] }),
                ),
                Err(e) => ok(
                    &id,
                    json!({ "content": [{ "type": "text", "text": e }], "isError": true }),
                ),
            })
        }
        "resources/list" => {
            let res: Vec<Value> = store
                .all_notes()
                .unwrap_or_default()
                .into_iter()
                .map(|n| {
                    json!({
                        "uri": format!("note://{}", n.id),
                        "name": crate::mdconv::title_from_html(&n.content),
                        "mimeType": "text/markdown"
                    })
                })
                .collect();
            Some(ok(&id, json!({ "resources": res })))
        }
        "resources/read" => {
            let uri = req
                .pointer("/params/uri")
                .and_then(|u| u.as_str())
                .unwrap_or("");
            match store.get_note(uri.strip_prefix("note://").unwrap_or("")) {
                Ok(Some(note)) => Some(ok(
                    &id,
                    json!({ "contents": [{ "uri": uri, "mimeType": "text/markdown", "text": crate::mdconv::html_to_md(&note.content) }] }),
                )),
                _ => Some(rpc_err(&id, -32602, "note not found")),
            }
        }
        _ => Some(rpc_err(&id, -32601, "method not found")),
    }
}

pub struct StoreAccess {
    pub app: AppHandle,
}

impl NoteStore for StoreAccess {
    fn all_notes(&self) -> Result<Vec<Note>, String> {
        let st = self.app.state::<Mutex<crate::storage::Store>>();
        let store = st.lock().unwrap();
        store.load_all_notes().map_err(|e| e.to_string())
    }
    fn get_note(&self, id: &str) -> Result<Option<Note>, String> {
        Ok(self.all_notes()?.into_iter().find(|n| n.id == id))
    }
    fn save(&self, note: &Note) -> Result<(), String> {
        let st = self.app.state::<Mutex<crate::storage::Store>>();
        {
            let store = st.lock().unwrap();
            store.save_note(note).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    fn set_folder(&self, id: &str, folder_id: Option<&str>) -> Result<(), String> {
        let st = self.app.state::<Mutex<crate::storage::Store>>();
        let store = st.lock().unwrap();
        store.set_folder(id, folder_id).map_err(|e| e.to_string())
    }
    fn set_archived(&self, id: &str, archived: bool) -> Result<(), String> {
        let st = self.app.state::<Mutex<crate::storage::Store>>();
        let store = st.lock().unwrap();
        store.set_archived(id, archived).map_err(|e| e.to_string())
    }
    fn trash(&self, id: &str, ts: i64) -> Result<(), String> {
        let st = self.app.state::<Mutex<crate::storage::Store>>();
        let store = st.lock().unwrap();
        store.trash_note(id, ts).map_err(|e| e.to_string())
    }
    fn untrash(&self, id: &str) -> Result<(), String> {
        let st = self.app.state::<Mutex<crate::storage::Store>>();
        let store = st.lock().unwrap();
        store.restore_note(id).map_err(|e| e.to_string())
    }
    fn list_folders(&self) -> Result<Vec<Folder>, String> {
        let st = self.app.state::<Mutex<crate::storage::Store>>();
        let store = st.lock().unwrap();
        crate::folders::load_folders(&store.conn).map_err(|e| e.to_string())
    }
    fn create_folder(&self, name: &str, parent_id: Option<&str>) -> Result<Folder, String> {
        let id = self.new_id();
        let st = self.app.state::<Mutex<crate::storage::Store>>();
        {
            let store = st.lock().unwrap();
            crate::folders::create_folder(&store.conn, &id, name, parent_id)
                .map_err(|e| e.to_string())?;
        }
        // Read the created row back so position/defaults are accurate.
        self.list_folders()?
            .into_iter()
            .find(|f| f.id == id)
            .ok_or_else(|| "folder creation failed".to_string())
    }
    fn now_ms(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }
    fn new_id(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }
    fn emit_changed(&self) {
        let _ = self.app.emit("notes-changed", ());
    }
}

struct McpState {
    app: AppHandle,
    token: String,
    auth_required: bool,
    allow_write: bool,
    version: String,
}

static SHUTDOWN: OnceLock<Mutex<Option<tokio::sync::oneshot::Sender<()>>>> = OnceLock::new();

async fn mcp_route(
    axum::extract::State(state): axum::extract::State<Arc<McpState>>,
    headers: axum::http::HeaderMap,
    axum::Json(req): axum::Json<serde_json::Value>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    if state.auth_required {
        let want = format!("Bearer {}", state.token);
        let ok = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .map(|h| h == want)
            .unwrap_or(false);
        if !ok {
            return (axum::http::StatusCode::UNAUTHORIZED, "unauthorized").into_response();
        }
    }
    let access = StoreAccess {
        app: state.app.clone(),
    };
    match handle_rpc(&req, &access, state.allow_write, &state.version) {
        Some(resp) => axum::Json(resp).into_response(),
        None => axum::http::StatusCode::ACCEPTED.into_response(),
    }
}

pub async fn apply(
    app: AppHandle,
    enabled: bool,
    bind: String,
    port: u16,
    token: String,
    auth_required: bool,
    allow_write: bool,
) -> Result<(), String> {
    if let Some(slot) = SHUTDOWN.get() {
        if let Some(tx) = slot.lock().unwrap().take() {
            let _ = tx.send(());
        }
    }
    if !enabled {
        return Ok(());
    }
    let host = if bind == "external" {
        "0.0.0.0"
    } else {
        "127.0.0.1"
    };
    let addr: std::net::SocketAddr = format!("{host}:{port}")
        .parse()
        .map_err(|e: std::net::AddrParseError| e.to_string())?;
    let version = app.package_info().version.to_string();
    let state = Arc::new(McpState {
        app,
        token,
        auth_required,
        allow_write,
        version,
    });
    let router = axum::Router::new()
        .route("/mcp", axum::routing::post(mcp_route))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| e.to_string())?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    SHUTDOWN
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap()
        .replace(tx);
    tauri::async_runtime::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = rx.await;
            })
            .await;
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    // `folders`/`seq` back `list_folders`/`create_folder`/`new_id`, which have
    // no test caller yet (their tools land in Tasks 7-11).
    #[allow(dead_code)]
    struct Fake {
        notes: std::sync::Mutex<Vec<Note>>,
        folders: std::sync::Mutex<Vec<Folder>>,
        seq: std::sync::Mutex<u64>,
    }
    impl NoteStore for Fake {
        fn all_notes(&self) -> Result<Vec<Note>, String> {
            Ok(self.notes.lock().unwrap().clone())
        }
        fn get_note(&self, id: &str) -> Result<Option<Note>, String> {
            Ok(self
                .notes
                .lock()
                .unwrap()
                .iter()
                .find(|n| n.id == id)
                .cloned())
        }
        fn save(&self, note: &Note) -> Result<(), String> {
            let mut v = self.notes.lock().unwrap();
            match v.iter_mut().find(|n| n.id == note.id) {
                // mirror save_note: only content/updated_at change on update
                Some(existing) => {
                    existing.content = note.content.clone();
                    existing.updated_at = note.updated_at;
                }
                None => v.push(note.clone()),
            }
            Ok(())
        }
        fn set_folder(&self, id: &str, folder_id: Option<&str>) -> Result<(), String> {
            let mut v = self.notes.lock().unwrap();
            let n = v.iter_mut().find(|n| n.id == id).ok_or("note not found")?;
            n.folder_id = folder_id.map(|s| s.to_string());
            Ok(())
        }
        fn set_archived(&self, id: &str, archived: bool) -> Result<(), String> {
            let mut v = self.notes.lock().unwrap();
            let n = v.iter_mut().find(|n| n.id == id).ok_or("note not found")?;
            n.archived = archived;
            Ok(())
        }
        fn trash(&self, id: &str, ts: i64) -> Result<(), String> {
            let mut v = self.notes.lock().unwrap();
            let n = v.iter_mut().find(|n| n.id == id).ok_or("note not found")?;
            n.deleted_at = Some(ts);
            Ok(())
        }
        fn untrash(&self, id: &str) -> Result<(), String> {
            let mut v = self.notes.lock().unwrap();
            let n = v.iter_mut().find(|n| n.id == id).ok_or("note not found")?;
            n.deleted_at = None;
            Ok(())
        }
        fn list_folders(&self) -> Result<Vec<Folder>, String> {
            Ok(self.folders.lock().unwrap().clone())
        }
        fn create_folder(&self, name: &str, parent_id: Option<&str>) -> Result<Folder, String> {
            let id = self.new_id();
            let f = Folder {
                id: id.clone(),
                name: name.to_string(),
                parent_id: parent_id.map(|s| s.to_string()),
                ..Default::default()
            };
            self.folders.lock().unwrap().push(f.clone());
            Ok(f)
        }
        fn now_ms(&self) -> i64 {
            1_000
        }
        fn new_id(&self) -> String {
            let mut s = self.seq.lock().unwrap();
            *s += 1;
            format!("id{s}")
        }
        fn emit_changed(&self) {}
    }

    fn fake() -> Fake {
        Fake {
            notes: std::sync::Mutex::new(vec![Note {
                id: "a".into(),
                content: "<p>Hello world</p>".into(),
                updated_at: 1,
                ..Default::default()
            }]),
            folders: std::sync::Mutex::new(vec![]),
            seq: std::sync::Mutex::new(0),
        }
    }
    fn call(method: &str, params: Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params })
    }

    #[test]
    fn initialize_reports_version_and_caps() {
        let r = handle_rpc(&call("initialize", json!({})), &fake(), false, "1.2.3").unwrap();
        assert_eq!(r["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(r["result"]["serverInfo"]["version"], "1.2.3");
        assert!(r["result"]["capabilities"]["tools"].is_object());
    }
    #[test]
    fn tools_list_has_five() {
        let r = handle_rpc(&call("tools/list", json!({})), &fake(), false, "v").unwrap();
        assert_eq!(r["result"]["tools"].as_array().unwrap().len(), 5);
    }
    #[test]
    fn notification_has_no_response() {
        assert!(handle_rpc(
            &call("notifications/initialized", json!({})),
            &fake(),
            false,
            "v"
        )
        .is_none());
    }
    #[test]
    fn html_text_roundtrip_helpers() {
        assert_eq!(html_to_text("<p>Hi</p><p>there</p>"), "Hi\nthere");
        assert_eq!(text_to_html("a\nb"), "<p>a</p><p>b</p>");
    }
}
