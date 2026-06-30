<h1 align="center">Notefix</h1>

<p align="center">
  A lightweight, local-first sticky-note desktop app.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white" alt="Tauri 2">
  <img src="https://img.shields.io/badge/React-19-61DAFB?logo=react&logoColor=white" alt="React 19">
  <img src="https://img.shields.io/badge/TypeScript-5-3178C6?logo=typescript&logoColor=white" alt="TypeScript 5">
  <img src="https://img.shields.io/badge/Rust-stable-000000?logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="MIT License">
</p>

---

**Notefix** is a fast desktop app for keeping rich-text notes close at hand. Everything
lives in a local SQLite database on your machine — no account, no cloud, no telemetry.
An optional server connection (coming in a future release) lets you sync across devices,
but the app is fully featured offline on its own.

## Features

- **Rich-text editor** — headings, bold/italic/underline, links, task lists, and
  syntax-highlighted code blocks, powered by [TipTap](https://tiptap.dev/). Paste or
  drag in images and resize them inline.
- **Folders & organization** — nested folders with custom icons and colors, pinned
  notes, color labels, an archive, and a trash with configurable retention.
- **Search & dashboard** — instant full-text search and a customizable, drag-and-drop
  dashboard with recent-notes, due-date, calendar, clock, and statistics widgets.
- **Revision history** — automatic content snapshots per note, so you can look back at
  earlier versions.
- **Export** — export a single note to Markdown, PDF, JPEG, Word (`.doc`), or plain
  text, or export notes in bulk with embedded images.
- **Native integration** — system tray, launch-at-login, frameless floating note
  windows, and deep links (`notefix://note/<id>`, `notefix://new`).
- **macOS widget** — a WidgetKit widget that surfaces your pinned and recent notes on
  the desktop.
- **MCP server** — an optional, token-protected [Model Context
  Protocol](https://modelcontextprotocol.io/) server so AI tools can read and create
  notes (with a separate write-permission toggle).
- **Optional sync** *(coming soon)* — connect to a Notefix server to sync notes across
  devices over OAuth 2.0 (PKCE); access tokens are stored in the OS keychain, never in
  plain text. The app works fully locally until you opt in.
- **Internationalized** — English, German, and French, auto-detected from your system
  language.

## Tech stack

- **[Tauri 2](https://tauri.app/)** (Rust backend) + a [React 19](https://react.dev/) +
  **TypeScript** frontend
- **[Tailwind CSS 4](https://tailwindcss.com/)** for styling
- **[TipTap](https://tiptap.dev/)** rich-text editor
- **[rusqlite](https://github.com/rusqlite/rusqlite)** (bundled SQLite) for local storage
- **[reqwest](https://github.com/seanmonstar/reqwest)** + **[tokio](https://tokio.rs/)**
  for networking, **[keyring](https://github.com/hwchen/keyring-rs)** for OS-keychain
  token storage, **[axum](https://github.com/tokio-rs/axum)** for the MCP server
- **[i18next](https://www.i18next.com/)** for translations

## Getting started

### Prerequisites

- [Node.js](https://nodejs.org/) 22 or newer
- The [Rust toolchain](https://www.rust-lang.org/tools/install) (stable)
- Tauri's platform dependencies — see the
  [Tauri prerequisites guide](https://v2.tauri.app/start/prerequisites/) (Xcode CLT on
  macOS; WebView2 on Windows; WebKitGTK and friends on Linux)

### Install & run

```bash
git clone https://github.com/NoiXdev/notefix.git
cd notefix
npm install
npm run tauri dev
```

`npm run tauri dev` launches the desktop app with hot-reload for the React frontend.
(`npm run dev` runs only the Vite frontend in a browser, without the Tauri shell.)

## Scripts

| Command | Description |
| --- | --- |
| `npm run tauri dev` | Run the full desktop app in development mode |
| `npm run dev` | Run only the Vite frontend (browser) |
| `npm run build` | Type-check and build the frontend bundle |
| `npm run tauri build` | Build the distributable desktop app |
| `npm run typecheck` | Type-check without emitting (`tsc --noEmit`) |
| `npm test` | Run the frontend test suite (Vitest) once |
| `npm run test:e2e` | Run the end-to-end tests (Playwright) |
| `npm run changelog` | Regenerate `CHANGELOG.md` from Conventional Commits |

Rust tests, formatting, and lints live under `src-tauri/`:

```bash
cd src-tauri
cargo test
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
```

## Building

```bash
npm run tauri build
```

Tauri produces platform-specific artifacts (`.dmg`/`.app` on macOS, an installer on
Windows, `.deb`/`.rpm`/AppImage on Linux). A local build is **unsigned**; macOS code
signing and notarization run automatically in CI when the Apple credentials are
present. See [`docs/macos-signing.md`](docs/macos-signing.md) for details.

## Project structure

```
src/                       # React frontend
├── App.tsx                # App shell & view routing
├── api.ts                 # Tauri IPC bindings + event listeners
├── components/            # Editor, note list, dashboard, settings, dialogs
├── hooks/                 # Notes/folders/settings state
├── export/                # Markdown / PDF / JPEG / Word exporters
└── i18n/                  # en / de / fr translations

src-tauri/                 # Rust backend (Tauri)
├── src/
│   ├── lib.rs             # App setup & command registration
│   ├── commands.rs        # IPC command handlers
│   ├── storage.rs         # SQLite store (notes, folders)
│   ├── profiles.rs        # Local/server context registry
│   ├── sync.rs / auth.rs  # Optional server sync + OAuth/PKCE
│   ├── mcp.rs             # Model Context Protocol server
│   ├── images.rs          # Image embedding, sharding & GC
│   ├── revisions.rs       # Note revision history
│   └── tray.rs / widgetshare.rs / export.rs / …
└── widget/                # macOS WidgetKit extension (Swift)
```

## License

[MIT](LICENSE) © noidee.dev
