// NOTE: handle_rpc/helpers are only exercised by tests in Task 1; the axum
// server + StoreAccess real impl that call them are wired in Task 2.
#![allow(dead_code)]

use serde_json::{json, Value};

pub trait NoteAccess: Send + Sync {
    fn list(&self) -> Vec<(String, String)>; // (id, title)
    fn get(&self, id: &str) -> Option<String>; // text
    fn search(&self, q: &str) -> Vec<(String, String)>;
    fn create(&self, content: &str) -> Result<String, String>; // -> id
    fn append(&self, id: &str, text: &str) -> Result<(), String>;
}

pub fn html_to_text(html: &str) -> String {
    let nl = regex::Regex::new(r"(?is)</(p|div|h[1-6]|li)>|<br\s*/?>").unwrap().replace_all(html, "\n");
    let stripped = regex::Regex::new(r"(?is)<[^>]+>").unwrap().replace_all(&nl, "");
    stripped.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").trim().to_string()
}

pub fn text_to_html(text: &str) -> String {
    if text.is_empty() { return "<p></p>".to_string(); }
    text.lines().map(|l| format!("<p>{}</p>", l.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;"))).collect()
}

fn ok(id: &Value, result: Value) -> Value { json!({ "jsonrpc": "2.0", "id": id, "result": result }) }
fn rpc_err(id: &Value, code: i64, msg: &str) -> Value { json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": msg } }) }

fn tool_defs() -> Value {
    json!([
        { "name": "list_notes", "description": "List all notes (id and title).", "inputSchema": { "type": "object", "properties": {} } },
        { "name": "get_note", "description": "Get a note's text by id.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } },
        { "name": "search_notes", "description": "Search notes by text (case-insensitive).", "inputSchema": { "type": "object", "properties": { "query": { "type": "string" } }, "required": ["query"] } },
        { "name": "create_note", "description": "Create a new note from text.", "inputSchema": { "type": "object", "properties": { "content": { "type": "string" } }, "required": ["content"] } },
        { "name": "append_note", "description": "Append text to a note.", "inputSchema": { "type": "object", "properties": { "id": { "type": "string" }, "text": { "type": "string" } }, "required": ["id", "text"] } }
    ])
}

fn call_tool(name: &str, args: &Value, store: &dyn NoteAccess, allow_write: bool) -> Result<String, String> {
    let s = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    match name {
        "list_notes" => Ok(store.list().into_iter().map(|(id, t)| format!("{id}\t{t}")).collect::<Vec<_>>().join("\n")),
        "get_note" => store.get(&s("id")).ok_or_else(|| "note not found".to_string()),
        "search_notes" => Ok(store.search(&s("query")).into_iter().map(|(id, t)| format!("{id}\t{t}")).collect::<Vec<_>>().join("\n")),
        "create_note" => { if !allow_write { return Err("writing disabled".into()); } store.create(&s("content")).map(|id| format!("created {id}")) }
        "append_note" => { if !allow_write { return Err("writing disabled".into()); } store.append(&s("id"), &s("text")).map(|_| "ok".to_string()) }
        _ => Err(format!("unknown tool {name}")),
    }
}

/// Returns None for notifications (no response).
pub fn handle_rpc(req: &Value, store: &dyn NoteAccess, allow_write: bool, version: &str) -> Option<Value> {
    let id = req.get("id").cloned().unwrap_or(Value::Null);
    match req.get("method").and_then(|m| m.as_str()).unwrap_or("") {
        "initialize" => Some(ok(&id, json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {}, "resources": {} },
            "serverInfo": { "name": "Notefix", "version": version }
        }))),
        "notifications/initialized" => None,
        "ping" => Some(ok(&id, json!({}))),
        "tools/list" => Some(ok(&id, json!({ "tools": tool_defs() }))),
        "tools/call" => {
            let name = req.pointer("/params/name").and_then(|n| n.as_str()).unwrap_or("");
            let args = req.pointer("/params/arguments").cloned().unwrap_or(json!({}));
            Some(match call_tool(name, &args, store, allow_write) {
                Ok(text) => ok(&id, json!({ "content": [{ "type": "text", "text": text }] })),
                Err(e) => ok(&id, json!({ "content": [{ "type": "text", "text": e }], "isError": true })),
            })
        }
        "resources/list" => {
            let res: Vec<Value> = store.list().into_iter().map(|(nid, title)| json!({ "uri": format!("note://{nid}"), "name": title, "mimeType": "text/markdown" })).collect();
            Some(ok(&id, json!({ "resources": res })))
        }
        "resources/read" => {
            let uri = req.pointer("/params/uri").and_then(|u| u.as_str()).unwrap_or("");
            match store.get(uri.strip_prefix("note://").unwrap_or("")) {
                Some(text) => Some(ok(&id, json!({ "contents": [{ "uri": uri, "mimeType": "text/plain", "text": text }] }))),
                None => Some(rpc_err(&id, -32602, "note not found")),
            }
        }
        _ => Some(rpc_err(&id, -32601, "method not found")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake { notes: std::sync::Mutex<Vec<(String, String)>> } // (id, text)
    impl NoteAccess for Fake {
        fn list(&self) -> Vec<(String, String)> { self.notes.lock().unwrap().iter().map(|(i, t)| (i.clone(), t.lines().next().unwrap_or("").to_string())).collect() }
        fn get(&self, id: &str) -> Option<String> { self.notes.lock().unwrap().iter().find(|(i, _)| i == id).map(|(_, t)| t.clone()) }
        fn search(&self, q: &str) -> Vec<(String, String)> { let ql = q.to_lowercase(); self.notes.lock().unwrap().iter().filter(|(_, t)| t.to_lowercase().contains(&ql)).map(|(i, t)| (i.clone(), t.lines().next().unwrap_or("").to_string())).collect() }
        fn create(&self, content: &str) -> Result<String, String> { self.notes.lock().unwrap().push(("new".into(), content.into())); Ok("new".into()) }
        fn append(&self, id: &str, text: &str) -> Result<(), String> { let mut n = self.notes.lock().unwrap(); let e = n.iter_mut().find(|(i, _)| i == id).ok_or("nf")?; e.1.push_str(text); Ok(()) }
    }
    fn fake() -> Fake { Fake { notes: std::sync::Mutex::new(vec![("a".into(), "Hello world".into())]) } }
    fn call(method: &str, params: Value) -> Value { json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params }) }

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
    fn search_filters() {
        let r = handle_rpc(&call("tools/call", json!({ "name": "search_notes", "arguments": { "query": "hello" } })), &fake(), false, "v").unwrap();
        assert!(r["result"]["content"][0]["text"].as_str().unwrap().contains("a"));
    }
    #[test]
    fn write_blocked_when_disabled() {
        let r = handle_rpc(&call("tools/call", json!({ "name": "create_note", "arguments": { "content": "x" } })), &fake(), false, "v").unwrap();
        assert_eq!(r["result"]["isError"], true);
    }
    #[test]
    fn write_allowed_when_enabled() {
        let r = handle_rpc(&call("tools/call", json!({ "name": "create_note", "arguments": { "content": "x" } })), &fake(), true, "v").unwrap();
        assert!(r["result"].get("isError").is_none());
    }
    #[test]
    fn notification_has_no_response() {
        assert!(handle_rpc(&call("notifications/initialized", json!({})), &fake(), false, "v").is_none());
    }
    #[test]
    fn html_text_roundtrip_helpers() {
        assert_eq!(html_to_text("<p>Hi</p><p>there</p>"), "Hi\nthere");
        assert_eq!(text_to_html("a\nb"), "<p>a</p><p>b</p>");
    }
}
