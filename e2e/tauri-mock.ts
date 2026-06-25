import type { Page } from '@playwright/test';

export interface MockData {
  notes?: unknown[];
  folders?: unknown[];
  trashed?: unknown[];
  settings?: [string, string][];
  responses?: Record<string, unknown>;
}

export async function installTauriMock(page: Page, data: MockData = {}): Promise<void> {
  await page.addInitScript((d: MockData) => {
    const singleContext = [{ id: 'local', label: '', kind: 'local', path: '/tmp/notefix.db', serverUrl: '', workspaceId: '', active: true }];
    const responses: Record<string, unknown> = {
      notes_load: d.notes ?? [],
      folders_load: d.folders ?? [],
      trash_load: d.trashed ?? [],
      settings_load: d.settings ?? [],
      note_stats: { notes: 0, archived: 0, characters: 0, words: 0 },
      get_db_path: '/x/notefix.db',
      note_revisions: [],
      check_paths: d.responses?.check_paths ?? { dbWritable: true, imagesWritable: true, dbPath: '/x', imagesPath: '/x/images' },
      // C0 contexts: list the single local context; add/rename/remove echo it; switch resolves void.
      contexts_list: d.responses?.contexts_list ?? singleContext,
      context_add: d.responses?.context_add ?? singleContext,
      context_rename: d.responses?.context_rename ?? singleContext,
      context_remove: d.responses?.context_remove ?? singleContext,
      context_switch: d.responses?.context_switch ?? undefined,
      // A1 add-server: begin returns an authorize URL; complete echoes contexts.
      server_auth_begin: d.responses?.server_auth_begin ?? 'https://srv.example/oauth/authorize?state=x',
      server_auth_complete: d.responses?.server_auth_complete ?? singleContext,
      // C1 sync: workspaces list, bind, status, manual trigger.
      server_workspaces: d.responses?.server_workspaces ?? [{ id: 'ws-1', name: 'Privat', role: 'owner' }],
      context_bind_workspace: d.responses?.context_bind_workspace ?? singleContext,
      sync_status: d.responses?.sync_status ?? { state: 'synced', lastSyncedAt: 1, pending: 0 },
      sync_now: undefined,
    };
    let cbId = 0;
    // Registry so tests can drive Tauri events (e.g. window "close-requested").
    const callbacks: Record<number, (e: unknown) => void> = {};
    const eventHandlers: Record<string, number[]> = {};
    const w = window as unknown as {
      __TAURI_INTERNALS__: Record<string, unknown>;
      __TAURI_EVENT_PLUGIN_INTERNALS__: Record<string, unknown>;
      __emitTauriEvent: (event: string, payload?: unknown) => void;
      __tauriCalls: string[];
    };
    // Log of invoked command names so tests can assert a command ran.
    w.__tauriCalls = [];
    // The event API calls this on unlisten/cleanup; without it the app throws
    // "Cannot read properties of undefined (reading 'unregisterListener')".
    w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    w.__TAURI_INTERNALS__ = {
      // getCurrentWindow()/getCurrentWebview() read these; without them they throw.
      metadata: {
        currentWindow: { label: 'main' },
        currentWebview: { windowLabel: 'main', label: 'main' },
      },
      invoke: async (cmd: string, args?: unknown) => {
        w.__tauriCalls.push(cmd);
        if (cmd in responses) return responses[cmd];
        if (cmd.startsWith('plugin:autostart')) return false;
        if (cmd === 'plugin:app|version') return '0.0.0-e2e';
        // Event plugin: remember which callback id handles which event so a test
        // can emit it; return an unlisten id. unlisten is a no-op.
        if (cmd === 'plugin:event|listen') {
          const a = (args ?? {}) as { event?: string; handler?: number };
          if (a.event && typeof a.handler === 'number') (eventHandlers[a.event] ??= []).push(a.handler);
          return ++cbId;
        }
        if (cmd === 'plugin:event|unlisten') return undefined;
        // Window/dialog/opener/process plugins are no-ops in the browser.
        if (cmd.startsWith('plugin:')) return undefined;
        return undefined;
      },
      transformCallback: (cb: unknown) => {
        const id = ++cbId;
        callbacks[id] = cb as (e: unknown) => void;
        return id;
      },
      unregisterCallback: () => {},
    };
    // Deliver an event to every listener registered for it (Tauri event shape).
    w.__emitTauriEvent = (event: string, payload?: unknown) => {
      for (const id of eventHandlers[event] ?? []) callbacks[id]?.({ event, id, payload });
    };
  }, data);
}
