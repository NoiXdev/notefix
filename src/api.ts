import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { isEnabled as autostartIsEnabled, enable as autostartEnable, disable as autostartDisable } from "@tauri-apps/plugin-autostart";
import { relaunch as processRelaunch } from "@tauri-apps/plugin-process";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { Note } from "./types";

export interface AppInfo {
  name: string;
  version: string;
  description: string;
}

export const api = {
  notes: {
    load: (): Promise<Note[]> => invoke("notes_load"),
    save: (note: Note): Promise<void> => invoke("notes_save", { note }),
    delete: (id: string): Promise<void> => invoke("notes_delete", { id }),
    setPinned: (id: string, pinned: boolean): Promise<void> =>
      invoke("notes_set_pinned", { id, pinned }),
    setArchived: (id: string, archived: boolean): Promise<void> =>
      invoke("notes_set_archived", { id, archived }),
    setColor: (id: string, color: string): Promise<void> =>
      invoke("notes_set_color", { id, color }),
    setDue: (id: string, dueAt: number | null): Promise<void> =>
      invoke("notes_set_due", { id, dueAt }),
    setFolder: (id: string, folderId: string | null): Promise<void> =>
      invoke("notes_set_folder", { id, folderId }),
    reorder: (folderId: string | null, ids: string[]): Promise<void> =>
      invoke("notes_reorder", { folderId, ids }),
    revisions: (noteId: string): Promise<import("./types").Revision[]> => invoke("note_revisions", { noteId }),
    revisionContent: (id: number): Promise<string | null> => invoke("note_revision_content", { id }),
    restore: (id: string): Promise<void> => invoke("notes_restore", { id }),
    purge: (id: string): Promise<void> => invoke("notes_purge", { id }),
  },

  trash: {
    load: (): Promise<import("./types").Note[]> => invoke("trash_load"),
    empty: (): Promise<void> => invoke("trash_empty"),
  },

  folders: {
    load: (): Promise<import("./types").Folder[]> => invoke("folders_load"),
    create: (id: string, name: string, parentId: string | null): Promise<void> =>
      invoke("folder_create", { id, name, parentId }),
    rename: (id: string, name: string): Promise<void> => invoke("folder_rename", { id, name }),
    move: (id: string, parentId: string | null): Promise<void> => invoke("folder_move", { id, parentId }),
    delete: (id: string, mode: "reparent" | "recursive"): Promise<void> => invoke("folder_delete", { id, mode }),
    reorder: (parentId: string | null, ids: string[]): Promise<void> =>
      invoke("folders_reorder", { parentId, ids }),
    setIcon: (id: string, icon: string): Promise<void> => invoke("folder_set_icon", { id, icon }),
    setColor: (id: string, color: string): Promise<void> => invoke("folder_set_color", { id, color }),
    setSort: (id: string, sort: string): Promise<void> => invoke("folder_set_sort", { id, sort }),
  },

  settings: {
    load: async (): Promise<Record<string, string>> =>
      Object.fromEntries(await invoke<[string, string][]>("settings_load")),
    set: (key: string, value: string): Promise<void> =>
      invoke("settings_set", { key, value }),
  },

  contexts: {
    list: (): Promise<import("./contexts").ContextInfo[]> => invoke("contexts_list"),
    add: (label: string): Promise<import("./contexts").ContextInfo[]> => invoke("context_add", { label }),
    switch: (id: string): Promise<void> => invoke("context_switch", { id }),
    rename: (id: string, label: string): Promise<import("./contexts").ContextInfo[]> =>
      invoke("context_rename", { id, label }),
    remove: (id: string, deleteFile: boolean): Promise<import("./contexts").ContextInfo[]> =>
      invoke("context_remove", { id, deleteFile }),
    /** Begin add-server: returns the browser authorize URL to open. */
    serverAuthBegin: (serverUrl: string): Promise<string> =>
      invoke("server_auth_begin", { serverUrl }),
    /** Complete add-server from a notefix://auth callback URL. */
    serverAuthComplete: (url: string): Promise<import("./contexts").ContextInfo[]> =>
      invoke("server_auth_complete", { url }),
  },

  saveImage: (noteId: string, name: string, bytes: number[]): Promise<string> => invoke("save_image", { noteId, name, bytes }),

  exportNotes: (path: string, ids: string[]): Promise<void> =>
    invoke("export_notes", { path, ids }),
  exportNotesBase64: (path: string, ids: string[]): Promise<void> => invoke("export_notes_base64", { path, ids }),
  exportNotesBundle: (dir: string, ids: string[]): Promise<void> => invoke("export_notes_bundle", { dir, ids }),

  noteInlinedHtml: (noteId: string): Promise<string> => invoke("note_inlined_html", { noteId }),
  saveExport: (path: string, bytes: number[]): Promise<void> => invoke("save_export", { path, bytes }),
  exportMdBundle: (dir: string, md: string, name: string): Promise<void> => invoke("export_md_bundle", { dir, md, name }),

  stats: (): Promise<import("./types").Stats> => invoke("note_stats"),

  getDbPath: (): Promise<string> => invoke("get_db_path"),
  setDbLocation: (folder: string): Promise<{ mode: "moved" | "switched"; path: string }> =>
    invoke("set_db_location", { folder }),
  quitApp: (): Promise<void> => invoke("quit_app"),
  hideMain: (): Promise<void> => invoke("hide_main"),
  onCloseRequested(callback: () => void): () => void {
    const unlisten = listen("close-requested", () => callback());
    return () => { void unlisten.then((un) => un()); };
  },

  relaunch: (): Promise<void> => processRelaunch(),
  pickFolder: async (): Promise<string | null> => {
    const r = await openDialog({ directory: true });
    return typeof r === "string" ? r : null;
  },

  autostart: {
    isEnabled: (): Promise<boolean> => autostartIsEnabled(),
    enable: (): Promise<void> => autostartEnable(),
    disable: (): Promise<void> => autostartDisable(),
  },

  /** Subscribe to cross-window note changes. Returns an unsubscribe fn. */
  onNotesChanged(callback: () => void): () => void {
    const unlisten = listen("notes-changed", () => callback());
    return () => {
      void unlisten.then((un) => un());
    };
  },

  /** Subscribe to active-context switches. Returns an unsubscribe fn. */
  onContextChanged(callback: () => void): () => void {
    const unlisten = listen("context-changed", () => callback());
    return () => {
      void unlisten.then((un) => un());
    };
  },

  /** Subscribe to `notefix://` auth-callback deep links. Returns an unsubscribe fn. */
  onAuthCallback(callback: (url: string) => void): () => void {
    const unlisten = listen<string>("auth-callback", (e) => callback(e.payload));
    return () => {
      void unlisten.then((un) => un());
    };
  },

  /** Subscribe to tray-menu actions. Returns an unsubscribe fn. */
  onTrayEvent(handlers: {
    newNote: () => void;
    openNote: (id: string) => void;
    openSettings: () => void;
  }): () => void {
    const subs = [
      listen("tray://new-note", () => handlers.newNote()),
      listen<string>("tray://open-note", (e) => handlers.openNote(e.payload)),
      listen("tray://open-settings", () => handlers.openSettings()),
    ];
    return () => {
      subs.forEach((p) => void p.then((un) => un()));
    };
  },

  openNoteWindow: (noteId: string): Promise<void> =>
    invoke("open_note_window", { noteId }),

  setWindowTitle: (title: string): Promise<void> =>
    getCurrentWindow().setTitle(title),

  /** Flip this window's always-on-top from the caller-known state; returns the new state. */
  toggleAlwaysOnTop: (current: boolean): Promise<boolean> =>
    getCurrentWindow()
      .setAlwaysOnTop(!current)
      .then(() => !current),

  /** Close the current window. DOM `window.close()` is a no-op in the webview, so route through Tauri. */
  closeWindow: (): Promise<void> => getCurrentWindow().close(),

  /** Start an interactive resize from the bottom-right corner (frameless window grip). */
  startResize: (): Promise<void> => getCurrentWindow().startResizeDragging("SouthEast"),

  checkPaths: (): Promise<{ dbWritable: boolean; imagesWritable: boolean; dbPath: string; imagesPath: string }> =>
    invoke("check_paths"),

  /** Harmless probe of the window:* capability group (set the current title to itself). */
  windowProbe: (): Promise<boolean> =>
    getCurrentWindow().title().then(t => getCurrentWindow().setTitle(t)).then(() => true).catch(() => false),

  getAppInfo: async (): Promise<AppInfo> => ({
    name: "Notefix",
    version: await getVersion(),
    description: "Simple better note app",
  }),

  /** Open an http/https link in the OS default browser. */
  openExternal: (url: string): Promise<void> => openUrl(url),

  fetchLinkMeta: (url: string): Promise<import("./linkMeta").LinkMeta> => invoke("fetch_link_meta", { url }),

  /** Start/stop/reconfigure the local MCP server. */
  mcpApplyConfig: (c: { enabled: boolean; bind: string; port: number; token: string; authRequired: boolean; allowWrite: boolean }): Promise<void> =>
    invoke("mcp_apply_config", c),
};
