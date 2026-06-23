import type { Page } from '@playwright/test';

export interface MockData {
  notes?: unknown[];
  folders?: unknown[];
  trashed?: unknown[];
  settings?: [string, string][];
}

export async function installTauriMock(page: Page, data: MockData = {}): Promise<void> {
  await page.addInitScript((d: MockData) => {
    const responses: Record<string, unknown> = {
      notes_load: d.notes ?? [],
      folders_load: d.folders ?? [],
      trash_load: d.trashed ?? [],
      settings_load: d.settings ?? [],
      note_stats: { notes: 0, archived: 0, characters: 0, words: 0 },
      get_db_path: '/x/notefix.db',
      note_revisions: [],
    };
    let cbId = 0;
    const w = window as unknown as {
      __TAURI_INTERNALS__: Record<string, unknown>;
      __TAURI_EVENT_PLUGIN_INTERNALS__: Record<string, unknown>;
    };
    // The event API calls this on unlisten/cleanup; without it the app throws
    // "Cannot read properties of undefined (reading 'unregisterListener')".
    w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    w.__TAURI_INTERNALS__ = {
      invoke: async (cmd: string) => {
        if (cmd in responses) return responses[cmd];
        if (cmd.startsWith('plugin:autostart')) return false;
        if (cmd === 'plugin:app|version') return '0.0.0-e2e';
        // Event plugin: listen returns an unlisten id, unlisten is a no-op.
        if (cmd === 'plugin:event|listen') return ++cbId;
        if (cmd === 'plugin:event|unlisten') return undefined;
        // Window/dialog/opener/process plugins are no-ops in the browser.
        if (cmd.startsWith('plugin:')) return undefined;
        return undefined;
      },
      transformCallback: (cb: unknown) => {
        void cb;
        return ++cbId;
      },
      unregisterCallback: () => {},
    };
  }, data);
}
