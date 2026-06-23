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
  },

  settings: {
    load: async (): Promise<Record<string, string>> =>
      Object.fromEntries(await invoke<[string, string][]>("settings_load")),
    set: (key: string, value: string): Promise<void> =>
      invoke("settings_set", { key, value }),
  },

  exportNotes: (path: string, ids: string[]): Promise<void> =>
    invoke("export_notes", { path, ids }),

  stats: (): Promise<import("./types").Stats> => invoke("note_stats"),

  getDbPath: (): Promise<string> => invoke("get_db_path"),
  setDbLocation: (folder: string): Promise<{ mode: "moved" | "switched"; path: string }> =>
    invoke("set_db_location", { folder }),
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

  getAppInfo: async (): Promise<AppInfo> => ({
    name: "Notefix",
    version: await getVersion(),
    description: "Simple better note app",
  }),

  /** Open an http/https link in the OS default browser. */
  openExternal: (url: string): Promise<void> => openUrl(url),
};
