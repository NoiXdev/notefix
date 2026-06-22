import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { isEnabled as autostartIsEnabled, enable as autostartEnable, disable as autostartDisable } from "@tauri-apps/plugin-autostart";
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
  },

  settings: {
    load: async (): Promise<Record<string, string>> =>
      Object.fromEntries(await invoke<[string, string][]>("settings_load")),
    set: (key: string, value: string): Promise<void> =>
      invoke("settings_set", { key, value }),
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
