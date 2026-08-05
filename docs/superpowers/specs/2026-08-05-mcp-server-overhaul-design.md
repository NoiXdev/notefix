# MCP Server Overhaul — Design

**Date:** 2026-08-05
**Status:** Approved (design), pending implementation plan

## Problem

Notefix ships an HTTP JSON-RPC MCP server (`src-tauri/src/mcp.rs`) that lets
local desktop AI clients (e.g. Claude Desktop) work with notes. Today it is too
thin and lossy:

1. **Markdown gets mangled on write.** Notes are stored as Tiptap HTML. The MCP
   server accepts plain text and converts it with a crude `text_to_html()` that
   just wraps each line in `<p>…</p>`. When a client sends Markdown
   (`# Heading`, `**bold**`, `- [ ] task`), the syntax is preserved literally as
   paragraph text and shows up escaped/unformatted in the app.
2. **No group information.** `list_notes`/`search_notes` return only `id` + title;
   the client can't see which folder (group) a note belongs to, and `create_note`
   can't place a note into a group.
3. **No content-type signal.** Responses don't declare what format the content is
   in, so the client has to guess.
4. **Lossy reads.** `get_note` flattens HTML to plain text, destroying formatting.
5. **Fragile output format.** `list_notes`/`search_notes` return tab-separated
   `"{id}\t{title}"` lines that the client must split by hand — brittle and not
   extensible.
6. **Incomplete lifecycle.** Only `create_note` and `append_note` exist. No way to
   edit (full replace), move, archive, trash, restore, or list archived/trashed
   notes; no way to create or discover groups.

## Goals

Turn the MCP server into a **Markdown-native, group-aware notes API** with a full
note lifecycle:

- Content crosses the MCP boundary as **Markdown** in both directions, converted
  to/from Tiptap HTML in Rust.
- Every list/get/search response is **JSON** (returned in the tool's text block).
- Groups (folders) are first-class: discoverable, targetable by id or name,
  returned on every note.
- Full lifecycle tools: create/append/update/archive/trash/restore notes;
  create/list groups; status-filtered listing.

## Non-goals (YAGNI)

- Setting `pinned` / `color` / `dueAt` on create.
- Tags (no such field exists in the model).
- Reconstructing custom link-preview node cards from MCP input.
- A persisted per-note "type" field (we use delivery-format metadata instead — no
  DB migration).
- Mobile considerations: the MCP server is desktop-only and already hidden on
  mobile; nothing changes there.

## Background / current state

- **Storage:** SQLite via `rusqlite`. Notes stored as Tiptap **HTML** in
  `notes.content`. No `content_type`/`format` column exists, and none is added.
- **Note model** (`src-tauri/src/storage.rs`, `src/types.ts`): `id`, `content`
  (HTML), `updatedAt`, `pinned`, `archived`, `color`, `dueAt`, `folderId`,
  `position`, `deletedAt`, `dirty`.
- **Groups = Folders** (`src-tauri/src/folders.rs`): `id`, `name`, `parentId`
  (hierarchical), `position`, `icon`, `color`, `sort`. A note's group membership is
  `notes.folder_id` (nullable; `NULL` = ungrouped/top-level).
- **Status is derived from fields**, not a single column:
  - `deletedAt != NULL` → **trashed** (soft-delete tombstone)
  - else `archived == true` → **archived**
  - else → **active**
- **Frontend markdown** (`src/markdown.ts`): `markdownToHtml` (marked + GFM +
  `fixTaskLists`/`restoreLinkPreviews`) and `htmlToMarkdown` (Turndown + GFM). The
  MCP server is Rust and **cannot** reuse this JS; it gets its own Rust converter.
- **Write gating:** an existing `allow_write` setting already gates
  `create_note`/`append_note` (`Err("writing disabled")` when off). All new write
  tools respect it.

## Architecture

### New module: `src-tauri/src/mdconv.rs`

Isolated, unit-tested Markdown↔HTML conversion, replacing the crude
`text_to_html`/`html_to_text` helpers currently inline in `mcp.rs`.

- **`md_to_html(md: &str) -> String`** — comrak with GFM extensions
  (tables, strikethrough, task list items, autolinks).
- **`html_to_md(html: &str) -> String`** — HTML → Markdown (via a Rust
  html-to-markdown crate; candidate: `htmd`). Used by `get_note`.
- **Task-list fidelity (hard requirement):** `- [ ]` / `- [x]` must round-trip to
  Notefix's Tiptap task-list structure
  `<ul data-type="taskList"><li data-type="taskItem" data-checked="true|false">…</li></ul>`,
  mirroring the frontend's `fixTaskLists`. comrak's default checkbox output
  (`<input type="checkbox">`) does **not** match Tiptap and must be post-processed
  into the `data-type` structure. Symmetrically, `html_to_md` must recognise the
  Tiptap task-list structure and emit `- [ ]` / `- [x]`.
- **`title_of(md_or_html)`** — first non-empty text line, used for the `title`
  field (matches today's behaviour).

**Contract & interface:** `mdconv` depends only on the conversion crates and knows
nothing about SQLite or MCP. Inputs are strings; outputs are strings. It can be
tested entirely in isolation.

### Content format at the boundary

- Reads return **Markdown** by default; `contentType` in the response echoes the
  format (`"markdown"`).
- Every **write** tool accepts an optional `format: "markdown" | "html" | "text"`
  (default `"markdown"`):
  - `markdown` → `md_to_html` before storing.
  - `html` → stored as-is (trusted passthrough for advanced clients).
  - `text` → literal text, wrapped safely (each line → `<p>`), for clients that
    explicitly want no Markdown interpretation.
- `get_note` accepts an optional `format` to request `html` or `text` instead of
  markdown; `contentType` in the response reflects what was returned.

### Response shape (JSON in the text block)

Note summary object (used by `list_notes`, `search_notes`):

```json
{
  "id": "uuid",
  "title": "First line of the note",
  "group": { "id": "uuid", "name": "Work", "path": "Work/Projects" },
  "contentType": "markdown",
  "status": "active",
  "updatedAt": 1234567890
}
```

- `group` is `null` for ungrouped notes. `path` is the folder name chain from root
  joined by `/`.
- `status` is one of `"active" | "archived" | "trashed"` (derived as above).
- `search_notes` items additionally include `"snippet": "…"`.

`get_note` returns a **full** object: the summary fields **plus** `content`
(markdown by default), `pinned` (bool), and `dueAt` (number | null).

`list_groups` returns an array of `{ "id", "name", "parentId": "uuid"|null, "path" }`.

Write tools return a small JSON confirmation, e.g. `create_note` →
`{ "id": "…", "group": { "id":"…", "name":"…" } | null, "status": "active" }`.
`archive_note`/`delete_note`/`restore_note` → `{ "id": "…", "status": "…" }`.
`create_group` → `{ "id": "…", "name": "…", "parentId": "…"|null, "path": "…" }`.

All JSON is returned as the tool result's `content[0].text` (a JSON string), the
same channel used today. Errors keep the existing shape
(`content: [{type:text,text:msg}], isError:true`).

### Group targeting (create_note / update_note)

A note can be placed/moved into a group via **either**:

- `groupId` — resolved exactly; error `"group not found"` if unknown.
- `groupName` — resolved **case-insensitively** against folder names. If exactly
  one match → use it. If none → error `"group not found"` (no accidental folder
  creation — use `create_group` first). If multiple → error listing the candidate
  ids, e.g. `"ambiguous group name 'Work': a1b2…, c3d4…"`.

Passing neither leaves the note ungrouped (`folder_id = NULL`). Passing both is an
error (`"specify groupId or groupName, not both"`).

## Tools (final set)

### Reads

| Tool | Input | Returns |
|------|-------|---------|
| `list_notes` | `status?` = `active`(default)`\|archived\|trashed\|all`, `groupId?` | JSON array of note summaries |
| `get_note` | `id` (req), `format?` | Full note JSON |
| `search_notes` | `query` (req), `status?` (default `active`), `groupId?` | JSON array of summaries + `snippet` |
| `list_groups` | — | JSON array of group objects |

### Writes (all gated by `allow_write`)

| Tool | Input | Effect / Returns |
|------|-------|------------------|
| `create_note` | `content` (req), `format?`, `groupId?`, `groupName?` | New note; returns id + group + status |
| `append_note` | `id` (req), `text` (req), `format?` | Appends converted content; returns `{id,status}` |
| `update_note` | `id` (req), `content?`, `format?`, `groupId?`, `groupName?` | Full-replace content and/or move to group; returns `{id,group,status}` |
| `create_group` | `name` (req), `parentId?` | New folder; returns group object |
| `archive_note` | `id` (req) | Sets `archived=true`; returns `{id,status:"archived"}` |
| `delete_note` | `id` (req) | Soft-delete (sets `deletedAt`) → trash; returns `{id,status:"trashed"}` |
| `restore_note` | `id` (req) | Returns note to active: clears `deletedAt` if trashed, sets `archived=false` if archived; returns `{id,status:"active"}` |

`update_note` with no `content` and only a group param performs a move-only. With
`content` present it replaces the whole note body.

All tool `description` and `inputSchema` strings are rewritten to be explicit,
notably stating **"content is Markdown (GFM)"** on `create_note`/`append_note`/
`update_note` so clients format correctly.

## Data flow examples

**Create a task note in a group:**
1. Client → `create_note { content: "# Groceries\n- [ ] milk\n- [x] eggs", groupName: "Home" }`.
2. Server resolves `groupName` "Home" → folder id.
3. `md_to_html` converts (task list → Tiptap `data-type` structure).
4. Note saved with `folder_id` set, UUID minted, `notes-changed` event emitted.
5. Returns `{ "id":"…", "group":{"id":"…","name":"Home","path":"Home"}, "status":"active" }`.

**Read it back:**
1. Client → `get_note { id }`.
2. `html_to_md` converts stored HTML back to Markdown (checkboxes → `- [ ]`/`- [x]`).
3. Returns full JSON with `content` in Markdown and `contentType: "markdown"`.

## Error handling

- Unknown note id → `"note not found"`.
- Unknown/ambiguous group → messages above.
- Writes while `allow_write` is off → existing `"writing disabled"`.
- Conflicting/invalid params → explicit messages (`groupId`+`groupName`, etc.).
- All errors returned via the existing `isError:true` text-content shape; the RPC
  envelope is unchanged.

## Testing

Rust unit tests (isolated where possible):

- **`mdconv` round-trips:** headings, bold/italic, ordered/unordered lists,
  **task lists** (checked & unchecked), inline code, code blocks, tables, links.
  Assert Tiptap task-list structure specifically.
- **Group resolution:** by id, by name (case-insensitive), not-found, ambiguous
  (multiple folders same name), both-params error.
- **Status filtering:** `list_notes` returns the right set for
  active/archived/trashed/all, and honours `groupId`.
- **Write gating:** each write tool refuses when `allow_write` is off.
- **JSON shape:** responses parse as JSON and carry the documented fields
  (`group`, `contentType`, `status`).

Project-wide gates stay green: `npx tsc --noEmit` and `npx vitest run` (i18n key
parity). No frontend changes are expected; verify regardless.

## Risks

- **Task-list conversion fidelity** is the main technical risk (comrak output vs
  Tiptap's `data-type` structure, both directions). Covered by dedicated tests; if
  a chosen crate can't be coaxed into the right structure, post-process the HTML.
- **Breaking contract change** (tab-separated → JSON, plain text → Markdown).
  Acceptable: the server only talks to the user's own local AI clients; no external
  consumers.
- **New Rust dependencies** (comrak + an html-to-markdown crate). Adds build
  weight; both are mature, widely-used crates.

## Out-of-scope / future

- Exposing note metadata (pinned/color/dueAt) as writable via MCP.
- MCP `structuredContent` + `outputSchema` (chose JSON-in-text for universal client
  support; can be layered on later without breaking the text payload).
