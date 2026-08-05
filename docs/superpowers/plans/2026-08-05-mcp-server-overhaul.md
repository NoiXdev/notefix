# MCP Server Overhaul Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn Notefix's MCP server into a Markdown-native, group-aware notes API with a full note lifecycle (create/append/update/archive/trash/restore, create/list groups), JSON responses, and a Rust Markdown↔HTML converter that preserves Tiptap task lists.

**Architecture:** Three layers inside `src-tauri/src/`. (1) A new pure `mdconv` module does Markdown↔HTML conversion with no knowledge of storage or MCP. (2) A redesigned `NoteStore` trait is the test seam: a real `StoreAccess` impl backed by SQLite + a `Fake` in-memory impl for tests. (3) The `call_tool` handler in `mcp.rs` orchestrates conversion, group resolution, status filtering, and JSON building on top of the trait. Content crosses the MCP boundary as Markdown; every list/get/search response is a JSON string in the tool's text block.

**Tech Stack:** Rust (edition 2021), axum JSON-RPC, rusqlite (SQLite), serde_json. New crates: `comrak` (Markdown→HTML, GFM) and `htmd` (HTML→Markdown). Frontend unchanged (TypeScript/React/Tiptap).

## Global Constraints

- Rust edition **2021**; crate is `notefix_lib` under `src-tauri/`.
- Rust tests run with: `cd src-tauri && cargo test`. A single test: `cargo test <name>`.
- Formatting/lint gates: `cd src-tauri && cargo fmt` and `cargo clippy --all-targets -- -D warnings` must pass.
- Frontend gates must stay green: from repo root `npx tsc --noEmit` and `npx vitest run`.
- The MCP server is **desktop-only** and already hidden on mobile — do not add mobile UI or gating; no frontend changes are expected in this plan.
- Notes are stored as **Tiptap HTML** in SQLite. There is **no** `content_type`/`format`/`isMarkdown` column and this plan adds none (no DB migration).
- Tiptap task-list HTML format (canonical round-trip target, matches `src/markdown.ts`):
  `<ul data-type="taskList"><li data-type="taskItem" data-checked="true|false">…</li></ul>`.
- Note status is **derived**, never a column: `deleted_at.is_some()` → `"trashed"`; else `archived` → `"archived"`; else `"active"`.
- `Store::save_note` upserts, but on an **existing** row its `ON CONFLICT` updates only `content`, `updated_at`, `dirty`. To change folder/archived/deleted you MUST use the dedicated methods (`set_folder`, `set_archived`, `trash_note`, `restore_note`). Never try to move/archive/trash a note by mutating a `Note` and calling `save_note`.
- All JSON responses are returned as the tool result's `content[0].text` (a serialized JSON string), preserving the existing `{ "content": [{ "type": "text", "text": … }] }` envelope. Errors keep the existing `isError: true` text-content shape.
- Response field names are **camelCase** (matches serde `rename_all = "camelCase"` and the TS types).

---

## File Structure

- **Create** `src-tauri/src/mdconv.rs` — pure Markdown↔HTML conversion + helpers. One responsibility: format conversion. No storage/MCP imports.
- **Modify** `src-tauri/src/mcp.rs` — redesigned `NoteStore` trait, rewritten `call_tool` handler, JSON builders, `StoreAccess` impl, `tool_defs`, resources handlers, and `#[cfg(test)]` module (`Fake` + handler tests).
- **Modify** `src-tauri/src/lib.rs` — add `mod mdconv;` (single line; verify `apply(...)` call is unaffected).
- **Modify** `src-tauri/Cargo.toml` — add `comrak` and `htmd` dependencies.
- **Unchanged** `storage.rs`, `folders.rs`, `migrate.rs`, `commands.rs` (we consume their existing APIs), and all of `src/` (frontend).

---

## Task 1: Add Markdown conversion dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml:20-43` (`[dependencies]`)

**Interfaces:**
- Produces: `comrak` and `htmd` crates available to the build.

- [ ] **Step 1: Add the crates**

Run from `src-tauri/`:

```bash
cargo add comrak htmd
```

This appends latest-compatible versions under `[dependencies]` and updates `Cargo.lock`.

- [ ] **Step 2: Verify the project still builds**

Run: `cd src-tauri && cargo build`
Expected: builds successfully (downloads `comrak`/`htmd` and transitive deps).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "build: add comrak + htmd for MCP markdown conversion"
```

---

## Task 2: `mdconv` — core Markdown → HTML

**Files:**
- Create: `src-tauri/src/mdconv.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod mdconv;` next to the other `mod` declarations)

**Interfaces:**
- Produces: `pub fn md_to_html(md: &str) -> String` — GFM Markdown → HTML. Task-list post-processing is added in Task 3; this task covers headings, emphasis, lists, code, blockquotes, tables, links, and hard line breaks.

- [ ] **Step 1: Register the module**

In `src-tauri/src/lib.rs`, add alongside the existing module declarations:

```rust
mod mdconv;
```

- [ ] **Step 2: Write the failing tests**

Create `src-tauri/src/mdconv.rs` with:

```rust
//! Pure Markdown <-> HTML conversion for the MCP boundary. Notes are stored as
//! Tiptap HTML; MCP speaks Markdown. Task-list handling mirrors `src/markdown.ts`
//! so notes created via MCP are indistinguishable from app-created ones.

use comrak::{markdown_to_html, ComrakOptions};

/// Markdown (GFM) -> HTML. Hard line breaks on (matches the frontend's
/// `marked` `breaks: true`), tables/strikethrough/autolinks/task items enabled.
pub fn md_to_html(md: &str) -> String {
    let mut opts = ComrakOptions::default();
    opts.extension.table = true;
    opts.extension.strikethrough = true;
    opts.extension.tasklist = true;
    opts.extension.autolink = true;
    opts.render.hardbreaks = true;
    let html = markdown_to_html(md, &opts);
    html.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_and_emphasis() {
        let h = md_to_html("# Title\n\nsome **bold** and *italic*");
        assert!(h.contains("<h1>Title</h1>"), "got: {h}");
        assert!(h.contains("<strong>bold</strong>"), "got: {h}");
        assert!(h.contains("<em>italic</em>"), "got: {h}");
    }

    #[test]
    fn bullet_and_ordered_lists() {
        let h = md_to_html("- a\n- b");
        assert!(h.contains("<ul>") && h.contains("<li>a</li>"), "got: {h}");
        let o = md_to_html("1. one\n2. two");
        assert!(o.contains("<ol>") && o.contains("<li>one</li>"), "got: {o}");
    }

    #[test]
    fn code_and_tables() {
        let c = md_to_html("`inline` and\n\n```\nblock\n```");
        assert!(c.contains("<code>inline</code>"), "got: {c}");
        assert!(c.contains("<pre>"), "got: {c}");
        let t = md_to_html("| a | b |\n|---|---|\n| 1 | 2 |");
        assert!(t.contains("<table>") && t.contains("<td>1</td>"), "got: {t}");
    }
}
```

- [ ] **Step 3: Run tests to verify they fail (then compile)**

Run: `cd src-tauri && cargo test --lib mdconv::tests`
Expected: the module fails to compile until `comrak` is wired (Task 1 done) — once it compiles, the three tests PASS. If any assertion fails on exact tag text, adjust the `assert!` substring to comrak's actual output (inspect the `got:` panic message); do NOT loosen a check to always-true.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib mdconv::tests`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/mdconv.rs src-tauri/src/lib.rs
git commit -m "feat(mcp): add mdconv module with core markdown->html"
```

---

## Task 3: `mdconv` task-list fidelity (Markdown → HTML)

**Files:**
- Modify: `src-tauri/src/mdconv.rs`

**Interfaces:**
- Consumes: `md_to_html` (Task 2).
- Produces: `md_to_html` output where GFM task items become Tiptap task lists — `comrak` emits `<li><input type="checkbox" ... /> text</li>` inside `<ul>`, which a private `tiptap_task_lists(html: &str) -> String` rewrites to `<ul data-type="taskList">` / `<li data-type="taskItem" data-checked="true|false">…</li>` with the `<input>` removed.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `src-tauri/src/mdconv.rs`:

```rust
    #[test]
    fn task_list_becomes_tiptap_structure() {
        let h = md_to_html("- [ ] todo\n- [x] done");
        // Tiptap task-list markers present:
        assert!(h.contains(r#"data-type="taskList""#), "got: {h}");
        assert!(
            h.contains(r#"data-type="taskItem" data-checked="false""#),
            "got: {h}"
        );
        assert!(
            h.contains(r#"data-type="taskItem" data-checked="true""#),
            "got: {h}"
        );
        // The raw checkbox <input> must be gone (Tiptap renders its own):
        assert!(!h.contains("<input"), "input not stripped: {h}");
        // Item text survives:
        assert!(h.contains("todo") && h.contains("done"), "got: {h}");
    }

    #[test]
    fn plain_list_is_not_marked_as_tasklist() {
        let h = md_to_html("- a\n- b");
        assert!(!h.contains("taskList"), "plain list wrongly tagged: {h}");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test --lib mdconv::tests::task_list_becomes_tiptap_structure`
Expected: FAIL (comrak output still has `<input>` and no `data-type`).

- [ ] **Step 3: Implement the task-list rewrite**

In `src-tauri/src/mdconv.rs`, add the helper and call it from `md_to_html` (replace the `let html = …; html.trim()…` tail):

```rust
use regex::Regex;
use std::sync::OnceLock;

/// Rewrite comrak's GFM checkbox output into Tiptap's task-list structure,
/// mirroring `fixTaskLists` in `src/markdown.ts`. comrak emits, per item:
///   <li><input type="checkbox" disabled="" /> text</li>   (unchecked)
///   <li><input type="checkbox" checked="" disabled="" /> text</li> (checked)
/// A <ul> containing any such <li> becomes data-type="taskList".
fn tiptap_task_lists(html: &str) -> String {
    static ITEM: OnceLock<Regex> = OnceLock::new();
    // Capture optional `checked` and the remaining item body.
    let item = ITEM.get_or_init(|| {
        Regex::new(r#"(?is)<li>\s*<input([^>]*?)type="checkbox"([^>]*?)/?>\s*(.*?)</li>"#).unwrap()
    });
    let mut out = html.to_string();
    // Only bother if there is at least one checkbox item.
    if !out.contains(r#"type="checkbox""#) {
        return out;
    }
    out = item
        .replace_all(&out, |c: &regex::Captures| {
            let attrs = format!("{}{}", &c[1], &c[2]);
            let checked = attrs.contains("checked");
            format!(
                r#"<li data-type="taskItem" data-checked="{}">{}</li>"#,
                if checked { "true" } else { "false" },
                c[3].trim()
            )
        })
        .to_string();
    // Tag any <ul> that now contains a taskItem. comrak groups consecutive task
    // items into one <ul>; mark every <ul> whose first item is a taskItem.
    static LIST: OnceLock<Regex> = OnceLock::new();
    let list = LIST.get_or_init(|| {
        Regex::new(r#"(?is)<ul>(\s*<li data-type="taskItem")"#).unwrap()
    });
    out = list
        .replace_all(&out, r#"<ul data-type="taskList">$1"#)
        .to_string();
    out
}
```

Then change the end of `md_to_html` to post-process:

```rust
    let html = markdown_to_html(md, &opts);
    tiptap_task_lists(&html).trim().to_string()
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib mdconv::tests`
Expected: PASS (all mdconv tests, incl. the two new ones). If the item regex misses due to comrak attribute ordering/whitespace, inspect the `got:` output and adjust the regex — the transformation, not the assertions, must change.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/mdconv.rs
git commit -m "feat(mcp): convert markdown task lists to Tiptap structure"
```

---

## Task 4: `mdconv` — HTML → Markdown (incl. task items)

**Files:**
- Modify: `src-tauri/src/mdconv.rs`

**Interfaces:**
- Produces: `pub fn html_to_md(html: &str) -> String` — Tiptap HTML → GFM Markdown. Task items (`<li data-checked="…">`) become `- [x]` / `- [ ]`; other content (headings, emphasis, lists, code, links, tables) uses `htmd`. A private `pre_tasks(html)` converts Tiptap task items into GFM checkbox `<li>`s so `htmd`/GFM emits the right markers; if `htmd`'s installed version doesn't emit `- [ ]` from `<input type=checkbox>`, use the string fallback shown.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module:

```rust
    #[test]
    fn html_to_md_basic() {
        let m = html_to_md("<h1>Title</h1><p>a <strong>b</strong></p>");
        assert!(m.contains("# Title"), "got: {m}");
        assert!(m.contains("**b**"), "got: {m}");
    }

    #[test]
    fn html_to_md_task_items() {
        let html = r#"<ul data-type="taskList"><li data-type="taskItem" data-checked="false">todo</li><li data-type="taskItem" data-checked="true">done</li></ul>"#;
        let m = html_to_md(html);
        assert!(m.contains("- [ ] todo"), "got: {m}");
        assert!(m.contains("- [x] done"), "got: {m}");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --lib mdconv::tests::html_to_md_task_items`
Expected: FAIL (`html_to_md` undefined).

- [ ] **Step 3: Implement `html_to_md`**

Add to `src-tauri/src/mdconv.rs`:

```rust
/// Rewrite Tiptap task items into GFM checkbox list items so the generic
/// HTML->Markdown pass emits `- [ ]` / `- [x]`. Inverse of `tiptap_task_lists`.
fn pre_tasks(html: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r#"(?is)<li[^>]*data-checked="(true|false)"[^>]*>(.*?)</li>"#).unwrap()
    });
    re.replace_all(html, |c: &regex::Captures| {
        let mark = if &c[1] == "true" { "x" } else { " " };
        format!(r#"<li><input type="checkbox"{}/> {}</li>"#,
            if &c[1] == "true" { " checked=\"\"" } else { "" },
            c[2].trim())
    })
    .to_string()
    .replace("<ul data-type=\"taskList\">", "<ul>")
}

/// Tiptap HTML -> GFM Markdown.
pub fn html_to_md(html: &str) -> String {
    let prepared = pre_tasks(html);
    htmd::convert(&prepared).unwrap_or_default().trim().to_string()
}
```

Note: this task's tests are the contract. `htmd::convert` is the expected entry point; if the installed `htmd` version exposes a builder/options API instead, adapt this one function to call it (e.g. `HtmlToMarkdown::builder().build().convert(&prepared)`). If the installed `htmd`/GFM does NOT render `<input type=checkbox>` into `- [ ]`/`- [x]`, replace the body of `html_to_md` with a direct pre-pass that turns each task `<li>` into a literal line, e.g. map `data-checked="true"` → `- [x] {text}` and `"false"` → `- [ ] {text}` before converting the rest — the two `html_to_md_task_items` assertions must pass either way.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib mdconv::tests`
Expected: PASS (all mdconv tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/mdconv.rs
git commit -m "feat(mcp): add html->markdown with task-item fidelity"
```

---

## Task 5: `mdconv` round-trip + boundary helpers

**Files:**
- Modify: `src-tauri/src/mdconv.rs`

**Interfaces:**
- Consumes: `md_to_html`, `html_to_md`.
- Produces:
  - `pub fn title_from_html(html: &str) -> String` — first non-empty text line (for the `title` field).
  - `pub fn wrap_plaintext(text: &str) -> String` — each line wrapped in `<p>…</p>` with HTML-escaping (for `format:"text"` writes; replaces the old `text_to_html`).

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module:

```rust
    #[test]
    fn round_trip_preserves_structure() {
        let md = "# Groceries\n\n- [ ] milk\n- [x] eggs";
        let back = html_to_md(&md_to_html(md));
        assert!(back.contains("# Groceries"), "got: {back}");
        assert!(back.contains("- [ ] milk"), "got: {back}");
        assert!(back.contains("- [x] eggs"), "got: {back}");
    }

    #[test]
    fn title_from_html_takes_first_line() {
        assert_eq!(title_from_html("<h1>Hello</h1><p>world</p>"), "Hello");
        assert_eq!(title_from_html("<p></p><p>Second</p>"), "Second");
        assert_eq!(title_from_html(""), "");
    }

    #[test]
    fn wrap_plaintext_escapes_and_wraps() {
        assert_eq!(wrap_plaintext("a\nb"), "<p>a</p><p>b</p>");
        assert_eq!(wrap_plaintext("<b>&"), "<p>&lt;b&gt;&amp;</p>");
        assert_eq!(wrap_plaintext(""), "<p></p>");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --lib mdconv::tests::title_from_html_takes_first_line`
Expected: FAIL (`title_from_html` undefined).

- [ ] **Step 3: Implement the helpers**

Add to `src-tauri/src/mdconv.rs`:

```rust
/// First non-empty text line of an HTML fragment — used as a note's title.
pub fn title_from_html(html: &str) -> String {
    html_to_md(html)
        .lines()
        .map(|l| l.trim_start_matches(['#', '-', '*', '>', ' ']).trim())
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .to_string()
}

/// Wrap literal text as HTML paragraphs, escaping markup. For `format:"text"`.
pub fn wrap_plaintext(text: &str) -> String {
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib mdconv::tests`
Expected: PASS (all mdconv tests). If `title_from_html` returns a stray markdown marker for some input, tighten the `trim_start_matches` set — the assertions define correct output.

- [ ] **Step 5: Run fmt + commit**

```bash
cd src-tauri && cargo fmt
git add src-tauri/src/mdconv.rs
git commit -m "feat(mcp): add title + plaintext helpers and round-trip tests"
```

---

## Task 6: Redesign the `NoteStore` trait (+ `Fake` + `StoreAccess`)

This task replaces the old `NoteAccess` trait (5 text-only methods) with a data-oriented `NoteStore` trait that returns `Note`/`Folder` structs and exposes the exact mutation primitives the handler needs. It updates the real `StoreAccess` impl and the test `Fake`. The `call_tool`/`handle_rpc` rewrite happens in Tasks 8–11; to keep this task compiling, `call_tool` is temporarily rewired to the new trait with its existing behavior preserved only enough to build (the read tools are reimplemented in Task 8).

**Files:**
- Modify: `src-tauri/src/mcp.rs` (trait at `:5-11`, `StoreAccess` at `:170-262`, `Fake` at `:358-406`)

**Interfaces:**
- Produces the trait:

```rust
use crate::folders::Folder;
use crate::storage::Note;

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
```

- [ ] **Step 1: Replace the trait definition**

In `src-tauri/src/mcp.rs`, replace the old `pub trait NoteAccess { … }` (lines 5-11) with the `NoteStore` trait above. Keep the `html_to_text`/`text_to_html` helpers for now (removed in Task 11).

- [ ] **Step 2: Rewrite `StoreAccess` to implement `NoteStore`**

Replace the entire `impl NoteAccess for StoreAccess { … }` block (lines 174-262) with:

```rust
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
```

- [ ] **Step 3: Rewrite the test `Fake` to implement `NoteStore`**

Replace the `Fake` struct and its `impl` (lines 358-406) with an in-memory implementation:

```rust
    use crate::folders::Folder;
    use crate::storage::Note;

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
            Ok(self.notes.lock().unwrap().iter().find(|n| n.id == id).cloned())
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
```

- [ ] **Step 4: Temporarily adapt `call_tool`/`handle_rpc`/resources to the new trait so it compiles**

Change the `call_tool` signature and `handle_rpc`/`mcp_route` parameter types from `&dyn NoteAccess` to `&dyn NoteStore`. Replace the body of `call_tool` with a minimal placeholder that returns JSON built from `all_notes()` (proper tools land in Tasks 8-11):

```rust
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
            Ok(json!(notes.iter().map(|n| json!({"id": n.id.clone()})).collect::<Vec<_>>()).to_string())
        }
        _ => Err(format!("unknown tool {name}")),
    }
}
```

Also update `resources/list` and `resources/read` in `handle_rpc` to use `store.all_notes()` (map to `note://{id}` with `title_from_html`; read returns `html_to_md`). Delete the tests that reference removed behavior only if they no longer compile — otherwise leave them; Task 8+ replace them. It is acceptable for this task to leave `tools/list` returning the old `tool_defs()` (rewritten in Task 11).

- [ ] **Step 5: Verify it compiles and existing structural tests pass**

Run: `cd src-tauri && cargo test --lib mcp`
Expected: compiles; `initialize_reports_version_and_caps` and `notification_has_no_response` PASS. Some old tool tests may be temporarily removed — that is expected and they are restored/replaced in later tasks.

- [ ] **Step 6: Commit**

```bash
cd src-tauri && cargo fmt
git add src-tauri/src/mcp.rs
git commit -m "refactor(mcp): data-oriented NoteStore trait + Fake/StoreAccess"
```

---

## Task 7: Handler helpers — status, group resolution, JSON builders

**Files:**
- Modify: `src-tauri/src/mcp.rs` (add pure helper fns above `call_tool`)

**Interfaces:**
- Produces (all take plain data, no `self`):
  - `fn status_of(n: &Note) -> &'static str`
  - `fn folder_path(folders: &[Folder], id: &str) -> String` — names from root joined by `/`.
  - `fn group_json(folders: &[Folder], folder_id: Option<&str>) -> Value` — `{id,name,path}` or `Value::Null`.
  - `fn resolve_group(store: &dyn NoteStore, group_id: Option<&str>, group_name: Option<&str>) -> Result<Option<String>, String>` — the id/name rules from the spec.
  - `fn note_summary(n: &Note, folders: &[Folder]) -> Value` and `fn note_full(n: &Note, folders: &[Folder], content: &str, content_type: &str) -> Value`.

- [ ] **Step 1: Write the failing tests**

Add a nested tests module (in the existing `#[cfg(test)] mod tests`):

```rust
    #[test]
    fn status_of_derives_from_fields() {
        let mut n = Note { id: "x".into(), ..Default::default() };
        assert_eq!(status_of(&n), "active");
        n.archived = true;
        assert_eq!(status_of(&n), "archived");
        n.deleted_at = Some(1);
        assert_eq!(status_of(&n), "trashed"); // trashed wins over archived
    }

    #[test]
    fn folder_path_walks_parents() {
        let folders = vec![
            Folder { id: "p".into(), name: "Work".into(), ..Default::default() },
            Folder { id: "c".into(), name: "Proj".into(), parent_id: Some("p".into()), ..Default::default() },
        ];
        assert_eq!(folder_path(&folders, "c"), "Work/Proj");
        assert_eq!(folder_path(&folders, "p"), "Work");
    }

    #[test]
    fn resolve_group_by_id_name_and_errors() {
        let s = fake();
        let f = s.create_folder("Home", None).unwrap();
        // by id
        assert_eq!(resolve_group(&s, Some(f.id.as_str()), None).unwrap(), Some(f.id.clone()));
        // by name (case-insensitive)
        assert_eq!(resolve_group(&s, None, Some("home")).unwrap(), Some(f.id.clone()));
        // neither -> None
        assert_eq!(resolve_group(&s, None, None).unwrap(), None);
        // unknown id
        assert!(resolve_group(&s, Some("nope"), None).is_err());
        // unknown name
        assert!(resolve_group(&s, None, Some("ghost")).is_err());
        // both -> error
        assert!(resolve_group(&s, Some(f.id.as_str()), Some("Home")).is_err());
    }

    #[test]
    fn resolve_group_ambiguous_name_errors() {
        let s = fake();
        s.create_folder("Dup", None).unwrap();
        s.create_folder("Dup", None).unwrap();
        let e = resolve_group(&s, None, Some("Dup")).unwrap_err();
        assert!(e.contains("ambiguous"), "got: {e}");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --lib mcp::tests::resolve_group_by_id_name_and_errors`
Expected: FAIL (helpers undefined).

- [ ] **Step 3: Implement the helpers**

Add above `call_tool` in `src-tauri/src/mcp.rs`:

```rust
fn status_of(n: &Note) -> &'static str {
    if n.deleted_at.is_some() {
        "trashed"
    } else if n.archived {
        "archived"
    } else {
        "active"
    }
}

fn folder_path(folders: &[Folder], id: &str) -> String {
    let mut names = Vec::new();
    let mut cur = Some(id.to_string());
    let mut guard = 0;
    while let Some(cid) = cur {
        guard += 1;
        if guard > 64 {
            break; // defensive against cycles
        }
        match folders.iter().find(|f| f.id == cid) {
            Some(f) => {
                names.push(f.name.clone());
                cur = f.parent_id.clone();
            }
            None => break,
        }
    }
    names.reverse();
    names.join("/")
}

fn group_json(folders: &[Folder], folder_id: Option<&str>) -> Value {
    match folder_id {
        Some(fid) => match folders.iter().find(|f| f.id == fid) {
            Some(f) => json!({ "id": f.id, "name": f.name, "path": folder_path(folders, fid) }),
            None => Value::Null,
        },
        None => Value::Null,
    }
}

fn resolve_group(
    store: &dyn NoteStore,
    group_id: Option<&str>,
    group_name: Option<&str>,
) -> Result<Option<String>, String> {
    match (group_id, group_name) {
        (Some(_), Some(_)) => Err("specify groupId or groupName, not both".into()),
        (Some(id), None) => {
            let folders = store.list_folders()?;
            if folders.iter().any(|f| f.id == id) {
                Ok(Some(id.to_string()))
            } else {
                Err("group not found".into())
            }
        }
        (None, Some(name)) => {
            let folders = store.list_folders()?;
            let matches: Vec<&Folder> = folders
                .iter()
                .filter(|f| f.name.to_lowercase() == name.to_lowercase())
                .collect();
            match matches.as_slice() {
                [] => Err("group not found".into()),
                [one] => Ok(Some(one.id.clone())),
                many => Err(format!(
                    "ambiguous group name '{}': {}",
                    name,
                    many.iter().map(|f| f.id.as_str()).collect::<Vec<_>>().join(", ")
                )),
            }
        }
        (None, None) => Ok(None),
    }
}

fn note_summary(n: &Note, folders: &[Folder]) -> Value {
    json!({
        "id": n.id,
        "title": crate::mdconv::title_from_html(&n.content),
        "group": group_json(folders, n.folder_id.as_deref()),
        "contentType": "markdown",
        "status": status_of(n),
        "updatedAt": n.updated_at,
    })
}

fn note_full(n: &Note, folders: &[Folder], content: &str, content_type: &str) -> Value {
    let mut v = note_summary(n, folders);
    v["content"] = json!(content);
    v["contentType"] = json!(content_type);
    v["pinned"] = json!(n.pinned);
    v["dueAt"] = json!(n.due_at);
    v
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib mcp::tests`
Expected: PASS (the four new helper tests).

- [ ] **Step 5: Commit**

```bash
cd src-tauri && cargo fmt
git add src-tauri/src/mcp.rs
git commit -m "feat(mcp): status/group-resolution/json-builder helpers"
```

---

## Task 8: Read tools — `list_notes`, `get_note`, `search_notes`, `list_groups`

**Files:**
- Modify: `src-tauri/src/mcp.rs` (`call_tool` read arms)

**Interfaces:**
- Consumes: Task 7 helpers, `mdconv::html_to_md`, `mdconv::html_to_text` (existing) for text format.
- Produces `call_tool` arms:
  - `list_notes` — args `status?` (`active` default / `archived` / `trashed` / `all`), `groupId?`. Returns JSON array of `note_summary`.
  - `get_note` — args `id` (req), `format?` (`markdown` default / `html` / `text`). Returns `note_full` JSON. Error `"note not found"`.
  - `search_notes` — args `query` (req), `status?` (default `active`), `groupId?`. Returns array of summaries each with `snippet`.
  - `list_groups` — no args. Returns JSON array of `{id,name,parentId,path}`.

- [ ] **Step 1: Write the failing tests**

Add to the tests module. Uses a helper to invoke a tool and parse JSON:

```rust
    fn call_json(store: &dyn NoteStore, name: &str, args: Value, allow_write: bool) -> Value {
        let text = call_tool(name, &args, store, allow_write).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn list_notes_returns_summaries_with_group_and_status() {
        let s = fake();
        let f = s.create_folder("Work", None).unwrap();
        s.save(&Note { id: "b".into(), content: "<p>Second</p>".into(), folder_id: Some(f.id.clone()), updated_at: 2, ..Default::default() }).unwrap();
        let arr = call_json(&s, "list_notes", json!({}), false);
        let items = arr.as_array().unwrap();
        assert_eq!(items.len(), 2); // "a" (seed) + "b", both active
        let b = items.iter().find(|i| i["id"] == "b").unwrap();
        assert_eq!(b["group"]["name"], "Work");
        assert_eq!(b["contentType"], "markdown");
        assert_eq!(b["status"], "active");
        assert!(b["title"].as_str().unwrap().contains("Second"));
    }

    #[test]
    fn list_notes_status_filter() {
        let s = fake();
        s.save(&Note { id: "arch".into(), content: "<p>x</p>".into(), archived: true, updated_at: 2, ..Default::default() }).unwrap();
        s.save(&Note { id: "del".into(), content: "<p>y</p>".into(), deleted_at: Some(9), updated_at: 3, ..Default::default() }).unwrap();
        let active = call_json(&s, "list_notes", json!({}), false);
        assert_eq!(active.as_array().unwrap().len(), 1); // only seed "a"
        let archived = call_json(&s, "list_notes", json!({"status":"archived"}), false);
        assert_eq!(archived.as_array().unwrap()[0]["id"], "arch");
        let trashed = call_json(&s, "list_notes", json!({"status":"trashed"}), false);
        assert_eq!(trashed.as_array().unwrap()[0]["id"], "del");
        let all = call_json(&s, "list_notes", json!({"status":"all"}), false);
        assert_eq!(all.as_array().unwrap().len(), 3);
    }

    #[test]
    fn list_notes_group_filter() {
        let s = fake();
        let f = s.create_folder("G", None).unwrap();
        s.save(&Note { id: "in".into(), content: "<p>x</p>".into(), folder_id: Some(f.id.clone()), updated_at: 2, ..Default::default() }).unwrap();
        let arr = call_json(&s, "list_notes", json!({"groupId": f.id}), false);
        let items = arr.as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "in");
    }

    #[test]
    fn get_note_returns_markdown_content() {
        let s = fake();
        s.save(&Note { id: "m".into(), content: "<h1>Title</h1><p>body</p>".into(), updated_at: 2, ..Default::default() }).unwrap();
        let v = call_json(&s, "get_note", json!({"id":"m"}), false);
        assert_eq!(v["contentType"], "markdown");
        assert!(v["content"].as_str().unwrap().contains("# Title"));
        assert!(v.get("pinned").is_some());
    }

    #[test]
    fn get_note_html_format() {
        let s = fake();
        let v = call_json(&s, "get_note", json!({"id":"a","format":"html"}), false);
        assert_eq!(v["contentType"], "html");
        assert!(v["content"].as_str().unwrap().contains("<p>Hello world</p>"));
    }

    #[test]
    fn get_note_missing_errors() {
        let s = fake();
        assert!(call_tool("get_note", &json!({"id":"zzz"}), &s, false).is_err());
    }

    #[test]
    fn search_notes_matches_and_snippets() {
        let s = fake();
        let arr = call_json(&s, "search_notes", json!({"query":"hello"}), false);
        let items = arr.as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "a");
        assert!(items[0]["snippet"].as_str().unwrap().to_lowercase().contains("hello"));
    }

    #[test]
    fn list_groups_returns_tree_fields() {
        let s = fake();
        let p = s.create_folder("P", None).unwrap();
        s.create_folder("C", Some(p.id.as_str())).unwrap();
        let arr = call_json(&s, "list_groups", json!({}), false);
        let items = arr.as_array().unwrap();
        assert_eq!(items.len(), 2);
        let c = items.iter().find(|i| i["name"] == "C").unwrap();
        assert_eq!(c["parentId"], p.id);
        assert_eq!(c["path"], "P/C");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --lib mcp::tests::list_notes_returns_summaries_with_group_and_status`
Expected: FAIL (placeholder `call_tool` only returns `{id}` objects / unknown tool).

- [ ] **Step 3: Implement the read arms**

Replace the placeholder `call_tool` read handling with (keep the `_ => Err(...)` default and the write arms from later tasks):

```rust
fn call_tool(
    name: &str,
    args: &Value,
    store: &dyn NoteStore,
    allow_write: bool,
) -> Result<String, String> {
    let s = |k: &str| args.get(k).and_then(|v| v.as_str()).map(|v| v.to_string());
    match name {
        "list_notes" => {
            let status = s("status").unwrap_or_else(|| "active".into());
            let group = s("groupId");
            let folders = store.list_folders()?;
            let items: Vec<Value> = store
                .all_notes()?
                .into_iter()
                .filter(|n| status == "all" || status_of(n) == status)
                .filter(|n| group.as_deref().map_or(true, |g| n.folder_id.as_deref() == Some(g)))
                .map(|n| note_summary(&n, &folders))
                .collect();
            Ok(json!(items).to_string())
        }
        "get_note" => {
            let id = s("id").unwrap_or_default();
            let n = store.get_note(&id)?.ok_or("note not found")?;
            let folders = store.list_folders()?;
            let fmt = s("format").unwrap_or_else(|| "markdown".into());
            let content = match fmt.as_str() {
                "html" => n.content.clone(),
                "text" => html_to_text(&n.content),
                _ => crate::mdconv::html_to_md(&n.content),
            };
            let ct = match fmt.as_str() {
                "html" => "html",
                "text" => "text",
                _ => "markdown",
            };
            Ok(note_full(&n, &folders, &content, ct).to_string())
        }
        "search_notes" => {
            let q = s("query").unwrap_or_default().to_lowercase();
            let status = s("status").unwrap_or_else(|| "active".into());
            let group = s("groupId");
            let folders = store.list_folders()?;
            let items: Vec<Value> = store
                .all_notes()?
                .into_iter()
                .filter(|n| status == "all" || status_of(n) == status)
                .filter(|n| group.as_deref().map_or(true, |g| n.folder_id.as_deref() == Some(g)))
                .filter_map(|n| {
                    let plain = html_to_text(&n.content);
                    if !plain.to_lowercase().contains(&q) || q.is_empty() {
                        return None;
                    }
                    let mut v = note_summary(&n, &folders);
                    v["snippet"] = json!(snippet(&plain, &q));
                    Some(v)
                })
                .collect();
            Ok(json!(items).to_string())
        }
        "list_groups" => {
            let folders = store.list_folders()?;
            let items: Vec<Value> = folders
                .iter()
                .map(|f| json!({
                    "id": f.id,
                    "name": f.name,
                    "parentId": f.parent_id,
                    "path": folder_path(&folders, &f.id),
                }))
                .collect();
            Ok(json!(items).to_string())
        }
        _ => Err(format!("unknown tool {name}")),
    }
}
```

Add the `snippet` helper near the other helpers:

```rust
/// A short window (~120 chars) of `plain` around the first occurrence of the
/// already-lowercased `q`, with leading/trailing ellipses when clipped.
fn snippet(plain: &str, q: &str) -> String {
    let chars: Vec<char> = plain.chars().collect();
    let lower = plain.to_lowercase();
    let Some(byte_idx) = lower.find(q) else {
        return chars.iter().take(120).collect::<String>().trim().to_string();
    };
    let start = lower[..byte_idx].chars().count().saturating_sub(40);
    let end = (start + 120).min(chars.len());
    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.push_str(chars[start..end].iter().collect::<String>().trim());
    if end < chars.len() {
        out.push('…');
    }
    out
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib mcp::tests`
Expected: PASS (all read-tool tests + Task 7 helper tests).

- [ ] **Step 5: Commit**

```bash
cd src-tauri && cargo fmt
git add src-tauri/src/mcp.rs
git commit -m "feat(mcp): JSON read tools with group/status filters"
```

---

## Task 9: Write tools — `create_note`, `append_note`, `update_note`

**Files:**
- Modify: `src-tauri/src/mcp.rs` (`call_tool` write arms)

**Interfaces:**
- Consumes: `mdconv::{md_to_html, wrap_plaintext}`, `resolve_group`, `NoteStore`.
- Produces `call_tool` arms (all return `Err("writing disabled")` when `!allow_write`):
  - `create_note` — `content` (req), `format?`, `groupId?`, `groupName?`. Inserts a note; returns `{id, group, status:"active"}`.
  - `append_note` — `id` (req), `text` (req), `format?`. Appends converted HTML; returns `{id, status}`.
  - `update_note` — `id` (req), `content?`, `format?`, `groupId?`, `groupName?`. Replaces content (if `content` present) and/or moves group (if a group param present); returns `{id, group, status}`.
- Shared helper `fn to_html(content: &str, format: Option<&str>) -> String`.

- [ ] **Step 1: Write the failing tests**

Add to the tests module:

```rust
    #[test]
    fn create_note_converts_markdown_and_sets_group() {
        let s = fake();
        let f = s.create_folder("Home", None).unwrap();
        let v = call_json(&s, "create_note", json!({"content":"# Hi\n- [ ] task","groupName":"Home"}), true);
        assert_eq!(v["status"], "active");
        assert_eq!(v["group"]["name"], "Home");
        let id = v["id"].as_str().unwrap().to_string();
        let stored = s.get_note(&id).unwrap().unwrap();
        assert!(stored.content.contains("<h1>Hi</h1>"), "got: {}", stored.content);
        assert!(stored.content.contains(r#"data-type="taskItem""#), "got: {}", stored.content);
        assert_eq!(stored.folder_id.as_deref(), Some(f.id.as_str()));
    }

    #[test]
    fn create_note_blocked_when_writing_disabled() {
        let s = fake();
        assert_eq!(call_tool("create_note", &json!({"content":"x"}), &s, false).unwrap_err(), "writing disabled");
    }

    #[test]
    fn create_note_unknown_group_errors() {
        let s = fake();
        assert!(call_tool("create_note", &json!({"content":"x","groupName":"ghost"}), &s, true).is_err());
    }

    #[test]
    fn append_note_appends_converted_html() {
        let s = fake();
        let _ = call_tool("append_note", &json!({"id":"a","text":"**more**"}), &s, true).unwrap();
        let stored = s.get_note("a").unwrap().unwrap();
        assert!(stored.content.contains("<p>Hello world</p>"), "got: {}", stored.content);
        assert!(stored.content.contains("<strong>more</strong>"), "got: {}", stored.content);
    }

    #[test]
    fn update_note_replaces_content_and_moves() {
        let s = fake();
        let f = s.create_folder("Dest", None).unwrap();
        let _ = call_tool("update_note", &json!({"id":"a","content":"## New","groupName":"Dest"}), &s, true).unwrap();
        let stored = s.get_note("a").unwrap().unwrap();
        assert!(stored.content.contains("<h2>New</h2>"), "got: {}", stored.content);
        assert_eq!(stored.folder_id.as_deref(), Some(f.id.as_str()));
    }

    #[test]
    fn update_note_move_only_keeps_content() {
        let s = fake();
        let f = s.create_folder("Dest", None).unwrap();
        let _ = call_tool("update_note", &json!({"id":"a","groupId": f.id}), &s, true).unwrap();
        let stored = s.get_note("a").unwrap().unwrap();
        assert_eq!(stored.content, "<p>Hello world</p>"); // unchanged
        assert_eq!(stored.folder_id.as_deref(), Some(f.id.as_str()));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --lib mcp::tests::create_note_converts_markdown_and_sets_group`
Expected: FAIL (`unknown tool create_note`).

- [ ] **Step 3: Implement the write arms + `to_html`**

Add the helper near the others:

```rust
fn to_html(content: &str, format: Option<&str>) -> String {
    match format.unwrap_or("markdown") {
        "html" => content.to_string(),
        "text" => crate::mdconv::wrap_plaintext(content),
        _ => crate::mdconv::md_to_html(content),
    }
}
```

Add these arms to the `match name` in `call_tool` (before the `_ =>` default):

```rust
        "create_note" => {
            if !allow_write {
                return Err("writing disabled".into());
            }
            let content = s("content").ok_or("content is required")?;
            let folder_id = resolve_group(store, s("groupId").as_deref(), s("groupName").as_deref())?;
            let note = Note {
                id: store.new_id(),
                content: to_html(&content, s("format").as_deref()),
                updated_at: store.now_ms(),
                folder_id: folder_id.clone(),
                ..Default::default()
            };
            store.save(&note)?;
            store.emit_changed();
            let folders = store.list_folders()?;
            Ok(json!({
                "id": note.id,
                "group": group_json(&folders, folder_id.as_deref()),
                "status": "active",
            })
            .to_string())
        }
        "append_note" => {
            if !allow_write {
                return Err("writing disabled".into());
            }
            let id = s("id").ok_or("id is required")?;
            let text = s("text").ok_or("text is required")?;
            let mut note = store.get_note(&id)?.ok_or("note not found")?;
            note.content.push_str(&to_html(&text, s("format").as_deref()));
            note.updated_at = store.now_ms();
            store.save(&note)?;
            store.emit_changed();
            Ok(json!({ "id": note.id, "status": status_of(&note) }).to_string())
        }
        "update_note" => {
            if !allow_write {
                return Err("writing disabled".into());
            }
            let id = s("id").ok_or("id is required")?;
            let mut note = store.get_note(&id)?.ok_or("note not found")?;
            if let Some(content) = s("content") {
                note.content = to_html(&content, s("format").as_deref());
                note.updated_at = store.now_ms();
                store.save(&note)?;
            }
            // Move only when a group param is present (either key).
            if args.get("groupId").is_some() || args.get("groupName").is_some() {
                let folder_id = resolve_group(store, s("groupId").as_deref(), s("groupName").as_deref())?;
                store.set_folder(&id, folder_id.as_deref())?;
                note.folder_id = folder_id;
            }
            store.emit_changed();
            let folders = store.list_folders()?;
            Ok(json!({
                "id": note.id,
                "group": group_json(&folders, note.folder_id.as_deref()),
                "status": status_of(&note),
            })
            .to_string())
        }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib mcp::tests`
Expected: PASS (all write-tool tests + earlier tests).

- [ ] **Step 5: Commit**

```bash
cd src-tauri && cargo fmt
git add src-tauri/src/mcp.rs
git commit -m "feat(mcp): markdown-aware create/append/update note tools"
```

---

## Task 10: Lifecycle tools — `create_group`, `archive_note`, `delete_note`, `restore_note`

**Files:**
- Modify: `src-tauri/src/mcp.rs` (`call_tool` arms)

**Interfaces:**
- Consumes: `NoteStore` mutation methods, `group_json`.
- Produces `call_tool` arms (all write-gated):
  - `create_group` — `name` (req), `parentId?`. Returns `{id,name,parentId,path}`.
  - `archive_note` — `id` (req). `set_archived(id,true)`; returns `{id,status:"archived"}`.
  - `delete_note` — `id` (req). `trash(id, now)`; returns `{id,status:"trashed"}`.
  - `restore_note` — `id` (req). `untrash(id)` + `set_archived(id,false)`; returns `{id,status:"active"}`.

- [ ] **Step 1: Write the failing tests**

Add to the tests module:

```rust
    #[test]
    fn create_group_returns_group_object() {
        let s = fake();
        let v = call_json(&s, "create_group", json!({"name":"Projects"}), true);
        assert_eq!(v["name"], "Projects");
        assert!(v["id"].as_str().unwrap().starts_with("id"));
        assert_eq!(v["path"], "Projects");
        assert_eq!(s.list_folders().unwrap().len(), 1);
    }

    #[test]
    fn archive_then_restore_roundtrip() {
        let s = fake();
        let a = call_json(&s, "archive_note", json!({"id":"a"}), true);
        assert_eq!(a["status"], "archived");
        assert!(s.get_note("a").unwrap().unwrap().archived);
        let r = call_json(&s, "restore_note", json!({"id":"a"}), true);
        assert_eq!(r["status"], "active");
        assert!(!s.get_note("a").unwrap().unwrap().archived);
    }

    #[test]
    fn delete_then_restore_roundtrip() {
        let s = fake();
        let d = call_json(&s, "delete_note", json!({"id":"a"}), true);
        assert_eq!(d["status"], "trashed");
        assert!(s.get_note("a").unwrap().unwrap().deleted_at.is_some());
        let r = call_json(&s, "restore_note", json!({"id":"a"}), true);
        assert_eq!(r["status"], "active");
        assert!(s.get_note("a").unwrap().unwrap().deleted_at.is_none());
    }

    #[test]
    fn lifecycle_tools_write_gated() {
        let s = fake();
        for t in ["create_group", "archive_note", "delete_note", "restore_note"] {
            assert_eq!(call_tool(t, &json!({"id":"a","name":"n"}), &s, false).unwrap_err(), "writing disabled");
        }
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test --lib mcp::tests::archive_then_restore_roundtrip`
Expected: FAIL (`unknown tool archive_note`).

- [ ] **Step 3: Implement the arms**

Add before the `_ =>` default in `call_tool`:

```rust
        "create_group" => {
            if !allow_write {
                return Err("writing disabled".into());
            }
            let name = s("name").ok_or("name is required")?;
            let parent = s("parentId");
            let f = store.create_folder(&name, parent.as_deref())?;
            let folders = store.list_folders()?;
            Ok(json!({
                "id": f.id,
                "name": f.name,
                "parentId": f.parent_id,
                "path": folder_path(&folders, &f.id),
            })
            .to_string())
        }
        "archive_note" => {
            if !allow_write {
                return Err("writing disabled".into());
            }
            let id = s("id").ok_or("id is required")?;
            store.set_archived(&id, true)?;
            store.emit_changed();
            Ok(json!({ "id": id, "status": "archived" }).to_string())
        }
        "delete_note" => {
            if !allow_write {
                return Err("writing disabled".into());
            }
            let id = s("id").ok_or("id is required")?;
            store.trash(&id, store.now_ms())?;
            store.emit_changed();
            Ok(json!({ "id": id, "status": "trashed" }).to_string())
        }
        "restore_note" => {
            if !allow_write {
                return Err("writing disabled".into());
            }
            let id = s("id").ok_or("id is required")?;
            store.untrash(&id)?;
            store.set_archived(&id, false)?;
            store.emit_changed();
            Ok(json!({ "id": id, "status": "active" }).to_string())
        }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test --lib mcp::tests`
Expected: PASS (all lifecycle tests + earlier).

- [ ] **Step 5: Commit**

```bash
cd src-tauri && cargo fmt
git add src-tauri/src/mcp.rs
git commit -m "feat(mcp): group creation + archive/trash/restore lifecycle"
```

---

## Task 11: Rewrite `tool_defs`, adapt resources, remove dead helpers

**Files:**
- Modify: `src-tauri/src/mcp.rs` (`tool_defs` at `:53-61`, resources handlers, old helpers/tests)

**Interfaces:**
- Produces: `tool_defs()` advertising all 11 tools with explicit descriptions and input schemas; resources handlers using the new trait; removal of `html_to_text`'s old sibling `text_to_html` if now unused (keep `html_to_text` — used by `get_note`/`search_notes` for the `text` format and snippet).

- [ ] **Step 1: Write the failing test**

Update/replace the old `tools_list_has_five` test with:

```rust
    #[test]
    fn tools_list_has_all_tools() {
        let r = handle_rpc(&call("tools/list", json!({})), &fake(), false, "v").unwrap();
        let names: Vec<String> = r["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        for expected in [
            "list_notes", "get_note", "search_notes", "list_groups",
            "create_note", "append_note", "update_note", "create_group",
            "archive_note", "delete_note", "restore_note",
        ] {
            assert!(names.contains(&expected.to_string()), "missing {expected}: {names:?}");
        }
        // create_note advertises that content is Markdown:
        let cn = r["result"]["tools"].as_array().unwrap().iter().find(|t| t["name"] == "create_note").unwrap();
        assert!(cn["description"].as_str().unwrap().to_lowercase().contains("markdown"));
    }

    #[test]
    fn resources_read_returns_markdown() {
        let s = fake();
        let r = handle_rpc(&call("resources/read", json!({"uri":"note://a"})), &s, false, "v").unwrap();
        assert!(r["result"]["contents"][0]["text"].as_str().unwrap().contains("Hello world"));
        assert_eq!(r["result"]["contents"][0]["mimeType"], "text/markdown");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test --lib mcp::tests::tools_list_has_all_tools`
Expected: FAIL (old `tool_defs` lists 5 tools with vague descriptions).

- [ ] **Step 3: Rewrite `tool_defs`**

Replace `tool_defs()` with:

```rust
fn tool_defs() -> Value {
    json!([
        { "name": "list_notes", "description": "List notes as JSON. Each item: {id, title, group, contentType, status, updatedAt}. Optional filters: status (active|archived|trashed|all, default active) and groupId.",
          "inputSchema": { "type": "object", "properties": {
            "status": { "type": "string", "enum": ["active", "archived", "trashed", "all"] },
            "groupId": { "type": "string" } } } },
        { "name": "get_note", "description": "Get one note as JSON including its content. Content is returned as Markdown by default (contentType 'markdown'); pass format 'html' or 'text' for other representations.",
          "inputSchema": { "type": "object", "properties": {
            "id": { "type": "string" },
            "format": { "type": "string", "enum": ["markdown", "html", "text"] } }, "required": ["id"] } },
        { "name": "search_notes", "description": "Search notes by text (case-insensitive). Returns JSON summaries with a snippet. Optional status (default active) and groupId filters.",
          "inputSchema": { "type": "object", "properties": {
            "query": { "type": "string" },
            "status": { "type": "string", "enum": ["active", "archived", "trashed", "all"] },
            "groupId": { "type": "string" } }, "required": ["query"] } },
        { "name": "list_groups", "description": "List groups (folders) as JSON: {id, name, parentId, path}. Use these ids/names to target a group when creating or moving notes.",
          "inputSchema": { "type": "object", "properties": {} } },
        { "name": "create_note", "description": "Create a note. 'content' is GitHub-Flavored Markdown by default and is converted to the app's rich format (headings, bold, lists, task lists, tables). Pass format 'html' or 'text' to override. Optionally place it in a group via groupId or groupName (groupName must match exactly one existing group).",
          "inputSchema": { "type": "object", "properties": {
            "content": { "type": "string" },
            "format": { "type": "string", "enum": ["markdown", "html", "text"] },
            "groupId": { "type": "string" },
            "groupName": { "type": "string" } }, "required": ["content"] } },
        { "name": "append_note", "description": "Append to a note. 'text' is Markdown by default (format 'html'/'text' to override) and is converted before appending.",
          "inputSchema": { "type": "object", "properties": {
            "id": { "type": "string" },
            "text": { "type": "string" },
            "format": { "type": "string", "enum": ["markdown", "html", "text"] } }, "required": ["id", "text"] } },
        { "name": "update_note", "description": "Replace a note's whole content (Markdown by default; format 'html'/'text' to override) and/or move it to another group via groupId or groupName. Omit 'content' to move only.",
          "inputSchema": { "type": "object", "properties": {
            "id": { "type": "string" },
            "content": { "type": "string" },
            "format": { "type": "string", "enum": ["markdown", "html", "text"] },
            "groupId": { "type": "string" },
            "groupName": { "type": "string" } }, "required": ["id"] } },
        { "name": "create_group", "description": "Create a group (folder). Optionally nest it under an existing group via parentId. Returns the new group {id, name, parentId, path}.",
          "inputSchema": { "type": "object", "properties": {
            "name": { "type": "string" },
            "parentId": { "type": "string" } }, "required": ["name"] } },
        { "name": "archive_note", "description": "Archive a note (moves it out of the active list; reversible with restore_note).",
          "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } },
        { "name": "delete_note", "description": "Move a note to trash (soft-delete; reversible with restore_note).",
          "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } },
        { "name": "restore_note", "description": "Restore a note to active from trash or archive.",
          "inputSchema": { "type": "object", "properties": { "id": { "type": "string" } }, "required": ["id"] } }
    ])
}
```

- [ ] **Step 4: Adapt the resources handlers**

In `handle_rpc`, update `resources/list` and `resources/read` to use the new trait (they may already be adapted in Task 6 — ensure the final form is):

```rust
        "resources/list" => {
            let notes = store.all_notes().unwrap_or_default();
            let res: Vec<Value> = notes
                .into_iter()
                .filter(|n| n.deleted_at.is_none())
                .map(|n| json!({
                    "uri": format!("note://{}", n.id),
                    "name": crate::mdconv::title_from_html(&n.content),
                    "mimeType": "text/markdown"
                }))
                .collect();
            Some(ok(&id, json!({ "resources": res })))
        }
        "resources/read" => {
            let uri = req.pointer("/params/uri").and_then(|u| u.as_str()).unwrap_or("");
            let nid = uri.strip_prefix("note://").unwrap_or("");
            match store.get_note(nid) {
                Ok(Some(n)) => Some(ok(&id, json!({ "contents": [{
                    "uri": uri, "mimeType": "text/markdown", "text": crate::mdconv::html_to_md(&n.content)
                }] }))),
                _ => Some(rpc_err(&id, -32602, "note not found")),
            }
        }
```

- [ ] **Step 5: Remove dead code**

If `text_to_html` (old inline helper) is no longer referenced anywhere, delete it and its `html_text_roundtrip_helpers` test assertion for it (keep the `html_to_text` half if still used, or move that assertion to cover only `html_to_text`). Run `cargo build` and fix any unused-import/dead-code warnings.

- [ ] **Step 6: Run the full mcp test suite**

Run: `cd src-tauri && cargo test --lib mcp`
Expected: PASS (all mcp tests, including `tools_list_has_all_tools` and `resources_read_returns_markdown`).

- [ ] **Step 7: Commit**

```bash
cd src-tauri && cargo fmt
git add src-tauri/src/mcp.rs
git commit -m "feat(mcp): advertise all 11 tools, markdown resources, cleanup"
```

---

## Task 12: Full verification

**Files:** none (verification only)

- [ ] **Step 1: Rust tests**

Run: `cd src-tauri && cargo test`
Expected: PASS (all tests across storage/folders/mcp/mdconv).

- [ ] **Step 2: Formatting + lint**

Run: `cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings`
Expected: no formatting diffs, no clippy errors. Fix any warnings inline.

- [ ] **Step 3: Frontend gates (must stay green)**

Run from repo root:

```bash
npx tsc --noEmit && npx vitest run
```

Expected: PASS (no frontend changes were made; this confirms nothing regressed).

- [ ] **Step 4: Manual smoke (optional but recommended)**

Build/run the desktop app, enable the MCP server with write access in settings, and from an MCP client: `create_note` with `"# Hello\n- [ ] task"` into a group by name, then `get_note` — confirm the returned content is Markdown with `- [ ] task` and the note renders formatted (heading + checkbox) in the app, in the correct group.

- [ ] **Step 5: Final commit (if any fixes were needed)**

```bash
git add -A
git commit -m "chore(mcp): verification fixups"
```

---

## Self-Review (spec coverage)

- **Markdown both ways** → Tasks 2–5 (`mdconv`), consumed by read (Task 8) and write (Task 9) tools.
- **Task-list fidelity** → Tasks 3–4 with explicit Tiptap-structure assertions; round-trip in Task 5.
- **Return the group** → `group_json`/`folder_path` (Task 7), on every summary/full note (Tasks 7–8).
- **Return the content type** → `contentType` field in `note_summary`/`note_full` (Task 7), reflecting requested format in `get_note` (Task 8).
- **create_note accepts content type** → `format` param + `to_html` (Task 9); advertised in `tool_defs` (Task 11).
- **Create note in a group** → `groupId`/`groupName` + `resolve_group` (Tasks 7, 9).
- **list_groups / create_group** → Tasks 8, 10.
- **update_note (replace + move)** → Task 9.
- **archive/delete/restore + status listing** → Tasks 8 (status filter) + 10 (verbs).
- **JSON responses** → all read tools (Task 8), write confirmations (Tasks 9–10).
- **allow_write gating** → every write/lifecycle arm (Tasks 9–10) with a dedicated gating test.
- **No DB migration / desktop-only / green frontend gates** → Global Constraints + Task 12.
