import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { OPEN_CONTEXTS_EVENT } from "./shortcuts";

vi.mock("@tiptap/react", () => {
  // Return a STABLE editor object so NoteEditor's `[note.id, editor]` effect
  // doesn't re-run every render (an unstable mock + setProgress => infinite loop).
  const editor = {
    isActive: () => false,
    chain: () => ({ focus: () => ({ toggleBold: () => ({ run: vi.fn() }) }) }),
    commands: { setContent: vi.fn(), focus: vi.fn(), setInvisibles: vi.fn() },
    getHTML: () => "<p></p>",
    isEditable: true,
  };
  return { useEditor: () => editor, EditorContent: () => null };
});
vi.mock("@tiptap/starter-kit", () => ({ default: { configure: () => ({}) } }));
vi.mock("@tiptap/extension-underline", () => ({ default: {} }));
vi.mock("@tiptap/extension-placeholder", () => ({ default: { configure: () => ({}) } }));
vi.mock("@tiptap/extension-task-list", () => ({ default: {} }));
vi.mock("@tiptap/extension-task-item", () => ({ default: { configure: () => ({}) } }));
vi.mock("./components/ResizableImage", () => ({
  ResizableImage: { configure: () => ({}) },
}));
// Dashboard's react-grid-layout measures width via ResizeObserver, which
// jsdom doesn't implement — same passthrough mock as Dashboard.test.tsx.
vi.mock("react-grid-layout/legacy", () => ({
  __esModule: true,
  default: ({ children }: { children: import("react").ReactNode }) => children,
  WidthProvider: (C: unknown) => C,
}));
vi.mock("./export", () => ({ exportSelected: vi.fn(), exportBase64: mockExportBase64, exportBundle: mockExportBundle }));
vi.mock("./export/exporters", () => ({ exportNote: mockExportNote }));

const { mockLoad, mockSave, mockDeleteFn, mockSetPinned, mockFoldersLoad, mockExportNote, mockExportBase64, mockExportBundle, cbs } = vi.hoisted(() => ({
  mockLoad: vi.fn(() => Promise.resolve([] as unknown[])),
  mockSave: vi.fn(() => Promise.resolve(undefined)),
  mockDeleteFn: vi.fn(() => Promise.resolve(undefined)),
  mockSetPinned: vi.fn(() => Promise.resolve(undefined)),
  mockFoldersLoad: vi.fn(() => Promise.resolve([] as unknown[])),
  mockExportNote: vi.fn(() => Promise.resolve(undefined)),
  mockExportBase64: vi.fn(() => Promise.resolve(undefined)),
  mockExportBundle: vi.fn(() => Promise.resolve(undefined)),
  // Captured event-callback registrations (onTrayEvent, onContextChanged, ...)
  // so tests can invoke them directly to simulate a backend-pushed event, the
  // same way the real Tauri event bus would call them. onContextChanged has
  // several independent subscribers (App's own two effects, ContextSwitcher,
  // ...) so it's a list, invoked all-at-once like the real broadcast would.
  cbs: {
    tray: null as null | { newNote?: () => void; openNote?: (id: string) => void; openSettings?: () => void },
    contextChanged: [] as Array<() => void>,
    closeRequested: null as null | (() => void),
    authCallback: null as null | ((url: string) => void),
  },
}));

vi.mock("./api", () => ({
  api: {
    notes: { load: mockLoad, loadOne: vi.fn(() => Promise.resolve('<p></p>')), loadAll: vi.fn(() => Promise.resolve([])), search: vi.fn(() => Promise.resolve([])), searchAll: vi.fn(() => Promise.resolve([])), save: mockSave, delete: mockDeleteFn, setPinned: mockSetPinned, setArchived: vi.fn(() => Promise.resolve()), setColor: vi.fn(), setDue: vi.fn(), setFolder: vi.fn(), reorder: vi.fn(), restore: vi.fn(() => Promise.resolve()), purge: vi.fn(() => Promise.resolve()), setMcpHidden: vi.fn(() => Promise.resolve()) },
    trash: { load: vi.fn(() => Promise.resolve([])), empty: vi.fn(() => Promise.resolve()) },
    folders: { load: mockFoldersLoad, create: vi.fn(() => Promise.resolve()), rename: vi.fn(), move: vi.fn(), delete: vi.fn(() => Promise.resolve()), reorder: vi.fn(), setIcon: vi.fn(), setColor: vi.fn(), setSort: vi.fn(), setMcpHidden: vi.fn(() => Promise.resolve()) },
    exportNotes: vi.fn(),
    stats: vi.fn(() => Promise.resolve({ notes: 0, archived: 0, characters: 0, words: 0 })),
    settings: { load: vi.fn(() => Promise.resolve({})), set: vi.fn() },
    autostart: { isEnabled: () => Promise.resolve(false), enable: vi.fn(), disable: vi.fn() },
    checkPaths: vi.fn(() => Promise.resolve({ dbWritable: true, imagesWritable: true, dbPath: '', imagesPath: '' })),
    windowProbe: vi.fn(() => Promise.resolve(true)),
    onTrayEvent: (h: typeof cbs.tray) => { cbs.tray = h; return () => {}; },
    onNotesChanged: () => () => {},
    onCloseRequested: (cb: () => void) => { cbs.closeRequested = cb; return () => {}; },
    onContextChanged: (cb: () => void) => { cbs.contextChanged.push(cb); return () => {}; },
    onAuthCallback: (cb: (url: string) => void) => { cbs.authCallback = cb; return () => {}; },
    onSyncStatus: () => () => {},
    contexts: {
      list: vi.fn(() => Promise.resolve([])),
      switch: vi.fn(() => Promise.resolve()),
      add: vi.fn(() => Promise.resolve()),
      remove: vi.fn(() => Promise.resolve()),
      rename: vi.fn(() => Promise.resolve()),
      serverAuthComplete: vi.fn(() => Promise.resolve([])),
      serverWorkspaces: vi.fn(() => Promise.resolve([])),
      bindWorkspace: vi.fn(() => Promise.resolve([])),
      syncNow: vi.fn(),
      syncStatus: vi.fn(() => Promise.resolve({ state: 'local', lastSyncedAt: 0, pending: 0 })),
    },
    quitApp: vi.fn(),
    hideMain: vi.fn(),
    openNoteWindow: vi.fn(),
    setWindowTitle: vi.fn(),
    toggleAlwaysOnTop: vi.fn(),
    closeWindow: vi.fn(),
    getAppInfo: vi.fn(() => Promise.resolve({ name: 'Notefix', version: '0.6.0', description: 'x' })),
    getDbPath: vi.fn(() => Promise.resolve('/tmp/notefix.db')),
    setDbLocation: vi.fn(() => Promise.resolve()),
    pickFolder: vi.fn(() => Promise.resolve(null)),
    relaunch: vi.fn(() => Promise.resolve()),
    checkForUpdate: vi.fn(() => Promise.resolve(null)),
    openExternal: vi.fn(),
    githubReleases: vi.fn(() => Promise.resolve([])),
    mcpApplyConfig: vi.fn(() => Promise.resolve()),
    vault: {
      status: vi.fn(() => Promise.resolve({ exists: false, unlocked: false, biometric: false })),
      setup: vi.fn(() => Promise.resolve([])),
      unlock: vi.fn(() => Promise.resolve()),
      unlockRecovery: vi.fn(() => Promise.resolve()),
      unlockBiometric: vi.fn(() => Promise.resolve()),
      lock: vi.fn(() => Promise.resolve()),
      changePassphrase: vi.fn(() => Promise.resolve()),
      rotate: vi.fn(() => Promise.resolve([])),
      rotationRedeem: vi.fn(() => Promise.resolve()),
      recoveryFollowup: vi.fn(() => Promise.resolve()),
      biometricAvailable: vi.fn(() => Promise.resolve(false)),
      biometricEnable: vi.fn(() => Promise.resolve()),
      biometricDisable: vi.fn(() => Promise.resolve()),
      protectNote: vi.fn(() => Promise.resolve()),
      lockFolder: vi.fn(() => Promise.resolve()),
    },
  },
}));

import App from "./App";
import { api } from "./api";
import type { VaultStatus } from "./types";

/**
 * A COMPLETE `VaultStatus`, so a test only has to spell out what it varies —
 * the same pattern `Settings.test.tsx` uses.
 *
 * Armed with `mockResolvedValue` (never `…Once`) in the global `beforeEach`
 * below: `useVault` refreshes on mount AND on every `context-changed`, so the
 * number of calls a test provokes is not something the test controls. A
 * `…Once` chain would run out and silently fall back to the module-level
 * default mid-test — the source of the flakes this replaces.
 */
const vaultStatus = (overrides: Partial<VaultStatus> = {}): VaultStatus => ({
  exists: false, unlocked: false, biometric: false, conflict: false,
  recoveryHolder: true, rotationCode: false, recoveryMissing: false, sealOutdated: false,
  ...overrides,
});

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(api.vault.status).mockResolvedValue(vaultStatus());
  mockLoad.mockResolvedValue([]);
  mockSave.mockResolvedValue(undefined);
  mockDeleteFn.mockResolvedValue(undefined);
  mockFoldersLoad.mockResolvedValue([]);
  cbs.tray = null;
  cbs.contextChanged = [];
  cbs.closeRequested = null;
  cbs.authCallback = null;
});

describe("App — empty state", () => {
  it("shows the empty state when no notes exist", async () => {
    render(<App />);
    await waitFor(() => expect(screen.getByText(/Wähle eine Notiz/i)).toBeInTheDocument());
  });

  it("renders the sidebar", async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByText("Notefix")).toBeInTheDocument();
      expect(screen.getByTitle("Neue Notiz")).toBeInTheDocument();
    });
  });
});

describe("App — creating notes", () => {
  it("creates a note and auto-selects it when + is clicked", async () => {
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.click(screen.getByTitle("Neue Notiz"));
    await waitFor(() => expect(screen.getByTitle("Fett")).toBeInTheDocument());
    expect(screen.queryByText(/Wähle eine Notiz/i)).not.toBeInTheDocument();
  });

  it("shows the new note in the sidebar list", async () => {
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.click(screen.getByTitle("Neue Notiz"));
    await waitFor(() => expect(screen.getByText("Ohne Titel")).toBeInTheDocument());
  });
});

describe("App — protect action routes through the vault dialogs", () => {
  it("routes 'Notiz sperren' through VaultSetup when no vault exists yet", async () => {
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.click(screen.getByTitle("Neue Notiz"));
    await waitFor(() => expect(screen.getByText("Ohne Titel")).toBeInTheDocument());

    fireEvent.contextMenu(screen.getByText("Ohne Titel"));
    fireEvent.click(screen.getByText("Notiz sperren"));

    expect(await screen.findByText("Tresor einrichten")).toBeInTheDocument();
  });
});

describe("App — deleting notes", () => {
  it("returns to empty state when the only note is deleted", async () => {
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.click(screen.getByTitle("Neue Notiz"));
    await waitFor(() => screen.getByTitle("Notiz löschen"));
    fireEvent.click(screen.getByTitle("Notiz löschen"));
    await waitFor(() => screen.getByText(/Endgültig löschen|In Papierkorb/));
    fireEvent.click(screen.getByText(/Endgültig löschen|In Papierkorb/));
    await waitFor(() => expect(screen.getByText(/Wähle eine Notiz/i)).toBeInTheDocument());
  });
});

describe("App — shortcuts", () => {
  it("Cmd+N creates a note", async () => {
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.keyDown(document.body, { key: "n", metaKey: true });
    await waitFor(() => expect(screen.getByTitle("Fett")).toBeInTheDocument());
  });
});

const protectedNoteMeta = {
  id: "n1",
  updatedAt: Date.now(),
  pinned: false,
  archived: false,
  color: "",
  dueAt: null,
  folderId: null,
  position: 0,
  deletedAt: null,
  preview: "",
  tasksDone: 0,
  tasksTotal: 0,
  protected: true,
};

describe("App — editor unlock gate for protected notes", () => {
  it("shows the locked placeholder instead of loading content, and Unlock opens the unlock dialog", async () => {
    vi.mocked(api.vault.status).mockResolvedValue(vaultStatus({ exists: true, unlocked: false }));
    mockLoad.mockResolvedValueOnce([protectedNoteMeta]);

    render(<App />);

    await waitFor(() => expect(screen.getByText("Diese Notiz ist geschützt")).toBeInTheDocument());
    expect(api.notes.loadOne).not.toHaveBeenCalled();

    fireEvent.click(screen.getByText("Entsperren"));
    await waitFor(() => expect(screen.getByText("Tresor entsperren")).toBeInTheDocument());
  });
});

describe("App — vaultLockScope 'perNote'", () => {
  it("shows the locked placeholder for a not-yet-revealed protected note even though the vault is unlocked", async () => {
    vi.mocked(api.vault.status).mockResolvedValue(vaultStatus({ exists: true, unlocked: true }));
    vi.mocked(api.settings.load).mockResolvedValueOnce({ vaultLockScope: "perNote" });
    mockLoad.mockResolvedValueOnce([protectedNoteMeta]);

    render(<App />);

    await waitFor(() => expect(screen.getByText("Diese Notiz ist geschützt")).toBeInTheDocument());
    expect(api.notes.loadOne).not.toHaveBeenCalled();
  });

  it("shows the editor for a protected note when vaultLockScope is 'session' and the vault is unlocked", async () => {
    vi.mocked(api.vault.status).mockResolvedValue(vaultStatus({ exists: true, unlocked: true }));
    vi.mocked(api.settings.load).mockResolvedValueOnce({ vaultLockScope: "session" });
    mockLoad.mockResolvedValueOnce([protectedNoteMeta]);

    render(<App />);

    await waitFor(() => expect(api.notes.loadOne).toHaveBeenCalled());
    expect(screen.queryByText("Diese Notiz ist geschützt")).not.toBeInTheDocument();
  });
});

// Round 2 / Important 1(b): the workspace rotated past every key this device
// holds, so the backend would refuse every seal. Show the note, refuse edits.
describe("App — protected notes are read-only behind an outdated key generation", () => {
  it("renders the note with the read-only banner and no toolbar", async () => {
    vi.mocked(api.vault.status).mockResolvedValue(
      vaultStatus({ exists: true, unlocked: true, sealOutdated: true }),
    );
    vi.mocked(api.settings.load).mockResolvedValueOnce({ vaultLockScope: "session" });
    mockLoad.mockResolvedValueOnce([protectedNoteMeta]);

    render(<App />);

    // The note is still readable — this is not the locked placeholder.
    await waitFor(() => expect(api.notes.loadOne).toHaveBeenCalled());
    expect(screen.queryByText("Diese Notiz ist geschützt")).not.toBeInTheDocument();
    // (dnd-kit renders its own live region with role="status", so scope the
    // query to the banner's own text.)
    const banner = (await screen.findByText("Schreibgeschützt")).closest("[role='status']")!;
    // No rotation code waiting, so the way out is a plain unlock.
    expect(banner).toHaveTextContent("Entsperre den Tresor erneut mit deinem Passwort");
    expect(screen.queryByTitle("Fett")).not.toBeInTheDocument();
  });

  it("routes the banner to the redeem dialog when a rotation code is waiting", async () => {
    vi.mocked(api.vault.status).mockResolvedValue(
      vaultStatus({ exists: true, unlocked: true, sealOutdated: true, rotationCode: true }),
    );
    vi.mocked(api.settings.load).mockResolvedValueOnce({ vaultLockScope: "session" });
    mockLoad.mockResolvedValueOnce([protectedNoteMeta]);

    render(<App />);

    const button = await screen.findByRole("button", { name: "Wechsel-Code eingeben" });
    fireEvent.click(button);
    expect(await screen.findByText("Der Tresorschlüssel hat sich geändert. Gib den Code ein, den du bekommen hast, um wieder hineinzukommen.")).toBeInTheDocument();
  });

  it("leaves an UNprotected note editable — only sealed content is affected", async () => {
    vi.mocked(api.vault.status).mockResolvedValue(
      vaultStatus({ exists: true, unlocked: true, sealOutdated: true }),
    );
    mockLoad.mockResolvedValueOnce([{ ...protectedNoteMeta, protected: false }]);

    render(<App />);

    await waitFor(() => expect(screen.getByTitle("Fett")).toBeInTheDocument());
    expect(screen.queryByText(/Schreibgeschützt/)).not.toBeInTheDocument();
  });

  it("keeps a protected note editable once the device has caught up", async () => {
    vi.mocked(api.vault.status).mockResolvedValue(
      vaultStatus({ exists: true, unlocked: true, sealOutdated: false }),
    );
    vi.mocked(api.settings.load).mockResolvedValueOnce({ vaultLockScope: "session" });
    mockLoad.mockResolvedValueOnce([protectedNoteMeta]);

    render(<App />);

    await waitFor(() => expect(screen.getByTitle("Fett")).toBeInTheDocument());
    expect(screen.queryByText(/Schreibgeschützt/)).not.toBeInTheDocument();
  });
});

// Round 2 / minor: dropping a plaintext note into a locked folder SEALS it,
// so the move can be refused exactly like an explicit protect. Swallowing that
// would leave the note looking moved when it never was.
describe("App — a refused move is reported, not swallowed", () => {
  it("explains an outdated key generation and restores the list", async () => {
    vi.mocked(api.vault.status).mockResolvedValue(
      vaultStatus({ exists: true, unlocked: true }),
    );
    vi.mocked(api.notes.setFolder).mockRejectedValueOnce(
      new Error("vault: key generation outdated — unlock with your passphrase"),
    );
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.click(screen.getByTitle("Neue Notiz"));
    await waitFor(() => expect(screen.getByText("Ohne Titel")).toBeInTheDocument());

    fireEvent.contextMenu(screen.getByText("Ohne Titel"));
    fireEvent.click(screen.getByText("Verschieben nach"));
    fireEvent.click(screen.getByText("— Root —"));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Dieses Gerät hat den neuesten Tresorschlüssel noch nicht.",
    );
    // A refused MOVE is not a refused protect — neutral heading.
    expect(screen.getByText("Verschieben nicht möglich")).toBeInTheDocument();
    expect(screen.queryByText("Schützen nicht möglich")).not.toBeInTheDocument();
    // The optimistic move was undone by the reload.
    await waitFor(() => expect(mockLoad).toHaveBeenCalledTimes(2));
  });
});

describe("App — lock vault shortcut", () => {
  it("Mod+Shift+L locks the vault when it exists and is unlocked", async () => {
    vi.mocked(api.vault.status).mockResolvedValue(vaultStatus({ exists: true, unlocked: true }));

    render(<App />);
    await waitFor(() => expect(screen.getByTitle("Neue Notiz")).toBeInTheDocument());

    // Retry the keydown: the vault status refresh (which the handler's
    // closure needs to see status.unlocked = true) lands asynchronously
    // after mount, so the very first dispatch can race it.
    await waitFor(() => {
      fireEvent.keyDown(document.body, { key: "l", metaKey: true, shiftKey: true });
      expect(api.vault.lock).toHaveBeenCalled();
    });
  });

  it("does nothing when the vault doesn't exist", async () => {
    render(<App />);

    await waitFor(() => expect(screen.getByTitle("Neue Notiz")).toBeInTheDocument());
    fireEvent.keyDown(document.body, { key: "l", metaKey: true, shiftKey: true });
    expect(api.vault.lock).not.toHaveBeenCalled();
  });
});

describe("App — auto-lock idle timer", () => {
  it("locks the vault after autoLockMinutes of inactivity", async () => {
    vi.mocked(api.vault.status).mockResolvedValue(vaultStatus({ exists: true, unlocked: true }));
    vi.mocked(api.settings.load).mockResolvedValueOnce({ autoLockIdle: "true", autoLockMinutes: "0.01" });

    render(<App />);

    await waitFor(() => expect(screen.getByTitle("Neue Notiz")).toBeInTheDocument());
    await waitFor(() => expect(api.vault.lock).toHaveBeenCalled(), { timeout: 3000 });
  });
});

describe("App — What's New on update", () => {
  it("shows the changelog dialog when the app updated and there are releases since lastSeenVersion", async () => {
    vi.mocked(api.settings.load).mockResolvedValueOnce({ lastSeenVersion: "0.5.0" });
    vi.mocked(api.getAppInfo).mockResolvedValueOnce({ name: "Notefix", version: "0.6.0", description: "x" });
    vi.mocked(api.githubReleases).mockResolvedValueOnce([
      { tagName: "v0.6.0", name: "v0.6.0 — Apps page", body: "Cool new stuff", publishedAt: "2026-08-20T00:00:00Z", prerelease: false },
      { tagName: "v0.5.1", name: "", body: "Minor fixes", publishedAt: "2026-07-01T00:00:00Z", prerelease: false },
    ]);

    render(<App />);

    await waitFor(() => expect(screen.getByText("Neu in dieser Version")).toBeInTheDocument());
    expect(screen.getByText("v0.6.0 — Apps page")).toBeInTheDocument();
    expect(screen.getByText("Cool new stuff")).toBeInTheDocument();
    expect(screen.getByText("v0.5.1")).toBeInTheDocument(); // falls back to tagName when name is empty

    fireEvent.click(screen.getAllByText("Schließen")[0]);
    await waitFor(() => expect(api.settings.set).toHaveBeenCalledWith("lastSeenVersion", "0.6.0"));
    expect(screen.queryByText("Neu in dieser Version")).not.toBeInTheDocument();
  });

  it("does not show the dialog on a fresh install (lastSeenVersion ''), only persists the current version", async () => {
    vi.mocked(api.settings.load).mockResolvedValueOnce({});
    vi.mocked(api.getAppInfo).mockResolvedValueOnce({ name: "Notefix", version: "0.6.0", description: "x" });

    render(<App />);

    await waitFor(() => expect(screen.getByTitle("Neue Notiz")).toBeInTheDocument());
    await waitFor(() => expect(api.settings.set).toHaveBeenCalledWith("lastSeenVersion", "0.6.0"));
    expect(api.githubReleases).not.toHaveBeenCalled();
    expect(screen.queryByText("Neu in dieser Version")).not.toBeInTheDocument();
  });

  it("does not show the dialog when whatsNewOnUpdate is disabled, even with a newer version", async () => {
    vi.mocked(api.settings.load).mockResolvedValueOnce({ lastSeenVersion: "0.5.0", whatsNewOnUpdate: "false" });
    vi.mocked(api.getAppInfo).mockResolvedValueOnce({ name: "Notefix", version: "0.6.0", description: "x" });

    render(<App />);

    await waitFor(() => expect(screen.getByTitle("Neue Notiz")).toBeInTheDocument());
    expect(api.githubReleases).not.toHaveBeenCalled();
    expect(screen.queryByText("Neu in dieser Version")).not.toBeInTheDocument();
  });

  it("persists the version silently when the releases fetch fails", async () => {
    vi.mocked(api.settings.load).mockResolvedValueOnce({ lastSeenVersion: "0.5.0" });
    vi.mocked(api.getAppInfo).mockResolvedValueOnce({ name: "Notefix", version: "0.6.0", description: "x" });
    vi.mocked(api.githubReleases).mockRejectedValueOnce(new Error("network down"));

    render(<App />);

    await waitFor(() => expect(api.settings.set).toHaveBeenCalledWith("lastSeenVersion", "0.6.0"));
    expect(screen.queryByText("Neu in dieser Version")).not.toBeInTheDocument();
  });

  it("persists the version without a dialog when there are no releases since lastSeenVersion", async () => {
    vi.mocked(api.settings.load).mockResolvedValueOnce({ lastSeenVersion: "0.5.0" });
    vi.mocked(api.getAppInfo).mockResolvedValueOnce({ name: "Notefix", version: "0.6.0", description: "x" });
    vi.mocked(api.githubReleases).mockResolvedValueOnce([]);

    render(<App />);

    await waitFor(() => expect(api.settings.set).toHaveBeenCalledWith("lastSeenVersion", "0.6.0"));
    expect(screen.queryByText("Neu in dieser Version")).not.toBeInTheDocument();
  });
});

describe("App — additional keyboard shortcuts", () => {
  it("Cmd+Shift+N creates a folder via the newFolder binding", async () => {
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.keyDown(document.body, { key: "n", metaKey: true, shiftKey: true });
    await waitFor(() => expect(api.folders.create).toHaveBeenCalled());
  });

  it("locks the vault via Mod+Shift+L even while a text input elsewhere has focus", async () => {
    vi.mocked(api.vault.status).mockResolvedValue(vaultStatus({ exists: true, unlocked: true }));
    render(<App />);
    await waitFor(() => expect(screen.getByTitle("Neue Notiz")).toBeInTheDocument());

    const input = document.createElement("input");
    document.body.appendChild(input);
    input.focus();
    await waitFor(() => {
      fireEvent.keyDown(input, { key: "l", metaKey: true, shiftKey: true });
      expect(api.vault.lock).toHaveBeenCalled();
    });
    input.remove();
  });

  it("ArrowDown/ArrowUp move the selection between notes", async () => {
    mockLoad.mockResolvedValueOnce([
      { id: "a", updatedAt: 3, pinned: false, archived: false, color: "", dueAt: null, folderId: null, position: 0, deletedAt: null, preview: "Alpha", tasksDone: 0, tasksTotal: 0, protected: false, title: "", mcpHidden: false },
      { id: "b", updatedAt: 2, pinned: false, archived: false, color: "", dueAt: null, folderId: null, position: 1, deletedAt: null, preview: "Beta", tasksDone: 0, tasksTotal: 0, protected: false, title: "", mcpHidden: false },
    ]);
    render(<App />);
    await waitFor(() => expect(screen.getByText("Alpha").closest("button")).toHaveClass("bg-gray-800"));

    fireEvent.keyDown(document.body, { key: "ArrowDown" });
    await waitFor(() => expect(screen.getByText("Beta").closest("button")).toHaveClass("bg-gray-800"));

    fireEvent.keyDown(document.body, { key: "ArrowUp" });
    await waitFor(() => expect(screen.getByText("Alpha").closest("button")).toHaveClass("bg-gray-800"));
  });

  it("Cmd+E archives the selected note", async () => {
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.click(screen.getByTitle("Neue Notiz"));
    await waitFor(() => expect(screen.getByText("Ohne Titel")).toBeInTheDocument());
    // Retried inside the waitFor, like the context-switch shortcut below: the
    // key handler is re-registered from an effect once the new note is
    // selected, so the very first dispatch can land on the older closure.
    await waitFor(() => {
      fireEvent.keyDown(document.body, { key: "e", metaKey: true });
      expect(api.notes.setArchived).toHaveBeenCalledWith(expect.any(String), true);
    });
  });

  it("Cmd+Shift+K switches to the next context", async () => {
    vi.mocked(api.contexts.list).mockResolvedValue([
      { id: "c1", label: "Local", kind: "local", path: "", serverUrl: "", workspaceId: "", active: true },
      { id: "c2", label: "Other", kind: "local", path: "", serverUrl: "", workspaceId: "", active: false },
    ]);
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    await waitFor(() => {
      fireEvent.keyDown(document.body, { key: "k", metaKey: true, shiftKey: true });
      expect(api.contexts.switch).toHaveBeenCalledWith("c2");
    });
  });

  it("Cmd+K dispatches the open-contexts event", async () => {
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    const handler = vi.fn();
    window.addEventListener(OPEN_CONTEXTS_EVENT, handler);
    fireEvent.keyDown(document.body, { key: "k", metaKey: true });
    expect(handler).toHaveBeenCalled();
    window.removeEventListener(OPEN_CONTEXTS_EVENT, handler);
  });

  it("Cmd+P opens the note finder, Cmd+P again closes it", async () => {
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.keyDown(document.body, { key: "p", metaKey: true });
    await waitFor(() => expect(screen.getByPlaceholderText("Notizen durchsuchen…")).toBeInTheDocument());
    fireEvent.keyDown(document.body, { key: "p", metaKey: true });
    await waitFor(() => expect(screen.queryByPlaceholderText("Notizen durchsuchen…")).not.toBeInTheDocument());
  });

  it("shortcuts are disabled while the settings panel is open", async () => {
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.click(screen.getByTitle("Mehr"));
    fireEvent.click(screen.getByText("Einstellungen"));
    await waitFor(() => expect(screen.getByText("MCP")).toBeInTheDocument());
    const saveCallsBefore = mockSave.mock.calls.length;
    fireEvent.keyDown(document.body, { key: "n", metaKey: true });
    expect(mockSave.mock.calls.length).toBe(saveCallsBefore);
  });
});

describe("App — context switching", () => {
  it("reloads notes/folders/settings and refreshes the vault when the active context changes", async () => {
    render(<App />);
    await waitFor(() => expect(cbs.contextChanged.length).toBeGreaterThan(0));
    const loadCallsBefore = mockLoad.mock.calls.length;
    const statusCallsBefore = vi.mocked(api.vault.status).mock.calls.length;

    // Simulate the real event bus: every subscriber (App's own handlers,
    // ContextSwitcher, ...) gets called.
    await act(async () => { cbs.contextChanged.forEach(fn => fn()); });

    await waitFor(() => expect(mockLoad.mock.calls.length).toBeGreaterThan(loadCallsBefore));
    expect(vi.mocked(api.vault.status).mock.calls.length).toBeGreaterThan(statusCallsBefore);
  });
});

describe("App — auth callback", () => {
  it("completes a server auth callback via contexts.serverAuthComplete", async () => {
    render(<App />);
    await waitFor(() => expect(cbs.authCallback).not.toBeNull());
    await act(async () => { cbs.authCallback!("notefix://auth?token=abc"); });
    await waitFor(() => expect(api.contexts.serverAuthComplete).toHaveBeenCalledWith("notefix://auth?token=abc"));
  });
});

describe("App — tray events", () => {
  it("newNote tray event creates and selects a note", async () => {
    render(<App />);
    await waitFor(() => expect(cbs.tray).not.toBeNull());
    await act(async () => { await cbs.tray!.newNote?.(); });
    await waitFor(() => expect(screen.getByTitle("Fett")).toBeInTheDocument());
  });

  it("openNote tray event closes settings and selects the note", async () => {
    mockLoad.mockResolvedValueOnce([
      { id: "n1", updatedAt: 1, pinned: false, archived: false, color: "", dueAt: null, folderId: null, position: 0, deletedAt: null, preview: "Hi there", tasksDone: 0, tasksTotal: 0, protected: false, title: "", mcpHidden: false },
    ]);
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.click(screen.getByTitle("Mehr"));
    fireEvent.click(screen.getByText("Einstellungen"));
    await waitFor(() => expect(screen.getByText("MCP")).toBeInTheDocument());

    await waitFor(() => expect(cbs.tray).not.toBeNull());
    act(() => cbs.tray!.openNote?.("n1"));

    expect(screen.queryByText("MCP")).not.toBeInTheDocument();
    await waitFor(() => expect(screen.getByTitle("Fett")).toBeInTheDocument());
  });
});

describe("App — close prompt", () => {
  it("shows the close dialog on a close request; Minimize hides the window and remembers the choice", async () => {
    render(<App />);
    await waitFor(() => expect(cbs.closeRequested).not.toBeNull());
    act(() => cbs.closeRequested!());

    expect(await screen.findByText("Notefix schließen")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Diese Wahl merken"));
    fireEvent.click(screen.getByText("In Menüleiste"));

    expect(api.hideMain).toHaveBeenCalled();
    expect(api.settings.set).toHaveBeenCalledWith("closeAction", "minimize");
    expect(screen.queryByText("Notefix schließen")).not.toBeInTheDocument();
  });

  it("Quit exits the app instead", async () => {
    render(<App />);
    await waitFor(() => expect(cbs.closeRequested).not.toBeNull());
    act(() => cbs.closeRequested!());
    fireEvent.click(await screen.findByText("Beenden"));
    expect(api.quitApp).toHaveBeenCalled();
  });

  it("Cancel dismisses the dialog without side effects", async () => {
    render(<App />);
    await waitFor(() => expect(cbs.closeRequested).not.toBeNull());
    act(() => cbs.closeRequested!());
    await screen.findByText("Notefix schließen");
    fireEvent.click(screen.getByText("Abbrechen"));
    expect(screen.queryByText("Notefix schließen")).not.toBeInTheDocument();
    expect(api.hideMain).not.toHaveBeenCalled();
    expect(api.quitApp).not.toHaveBeenCalled();
  });
});

describe("App — system-check modal", () => {
  it("shows the modal when a system check fails and opens diagnostics settings", async () => {
    vi.mocked(api.checkPaths).mockResolvedValueOnce({ dbWritable: false, imagesWritable: true, dbPath: "/x", imagesPath: "/y" });
    render(<App />);
    expect(await screen.findByText("Systemprüfung")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Einstellungen öffnen"));
    await waitFor(() => expect(screen.queryByText("Systemprüfung")).not.toBeInTheDocument());
    expect(screen.getByRole("heading", { name: "Diagnose" })).toBeInTheDocument();
  });

  it("Close dismisses the modal without opening settings", async () => {
    vi.mocked(api.checkPaths).mockResolvedValueOnce({ dbWritable: false, imagesWritable: true, dbPath: "/x", imagesPath: "/y" });
    render(<App />);
    expect(await screen.findByText("Systemprüfung")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Schließen"));
    expect(screen.queryByText("Systemprüfung")).not.toBeInTheDocument();
  });
});

describe("App — workspace-picker bind prompt", () => {
  it("prompts to bind a workspace for an unbound server context, and binds on pick", async () => {
    vi.mocked(api.contexts.list).mockResolvedValue([
      { id: "srv1", label: "Server", kind: "server", path: "", serverUrl: "https://x", workspaceId: "", active: true },
    ]);
    vi.mocked(api.contexts.serverWorkspaces).mockResolvedValueOnce([{ id: "w1", name: "WS1", role: "owner" }]);
    render(<App />);
    await waitFor(() => expect(screen.getByText("Workspace wählen")).toBeInTheDocument());
    await waitFor(() => expect(screen.getByText("WS1")).toBeInTheDocument());
    fireEvent.click(screen.getByText("WS1"));
    await waitFor(() => expect(api.contexts.bindWorkspace).toHaveBeenCalledWith("srv1", "w1", "WS1"));
    await waitFor(() => expect(screen.queryByText("Workspace wählen")).not.toBeInTheDocument());
  });
});

describe("App — export flows", () => {
  it("note context menu 'Exportieren' opens the format modal; picking Markdown exports and closes it", async () => {
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.click(screen.getByTitle("Neue Notiz"));
    await waitFor(() => expect(screen.getByText("Ohne Titel")).toBeInTheDocument());
    fireEvent.contextMenu(screen.getByText("Ohne Titel"));
    fireEvent.click(screen.getByText("Exportieren"));
    expect(await screen.findByText("Notiz exportieren")).toBeInTheDocument();

    fireEvent.click(screen.getByText("Markdown"));

    expect(screen.queryByText("Notiz exportieren")).not.toBeInTheDocument();
    expect(mockExportNote).toHaveBeenCalledWith(expect.objectContaining({ id: expect.any(String) }), "md", false);
  });

  it("cancelling the export-format modal does not export", async () => {
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.click(screen.getByTitle("Neue Notiz"));
    await waitFor(() => expect(screen.getByText("Ohne Titel")).toBeInTheDocument());
    fireEvent.contextMenu(screen.getByText("Ohne Titel"));
    fireEvent.click(screen.getByText("Exportieren"));
    expect(await screen.findByText("Notiz exportieren")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Abbrechen"));
    expect(screen.queryByText("Notiz exportieren")).not.toBeInTheDocument();
    expect(mockExportNote).not.toHaveBeenCalled();
  });

  it("Settings 'Alle als JSON exportieren' opens the bulk export dialog; Bundle exports and closes it", async () => {
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.click(screen.getByTitle("Mehr"));
    fireEvent.click(screen.getByText("Einstellungen"));
    fireEvent.click(screen.getByText("System"));
    fireEvent.click(screen.getByText("Alle als JSON exportieren"));
    expect(await screen.findByText("Export")).toBeInTheDocument();

    fireEvent.click(screen.getByText("Als Bundle (Ordner mit Bildern)"));

    expect(screen.queryByText("Export")).not.toBeInTheDocument();
    expect(mockExportBundle).toHaveBeenCalledWith([]);
  });

  it("cancelling the bulk export dialog does not export", async () => {
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.click(screen.getByTitle("Mehr"));
    fireEvent.click(screen.getByText("Einstellungen"));
    fireEvent.click(screen.getByText("System"));
    fireEvent.click(screen.getByText("Alle als JSON exportieren"));
    expect(await screen.findByText("Export")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Abbrechen"));
    expect(screen.queryByText("Export")).not.toBeInTheDocument();
    expect(mockExportBase64).not.toHaveBeenCalled();
    expect(mockExportBundle).not.toHaveBeenCalled();
  });
});

describe("App — folder delete modal", () => {
  it("deletes an empty folder directly, without showing the modal", async () => {
    mockFoldersLoad.mockResolvedValue([
      { id: "f1", name: "Empty", parentId: null, position: 0, icon: "", color: "", sort: "manual", locked: false, mcpHidden: false },
    ]);
    render(<App />);
    await waitFor(() => expect(screen.getByText("Empty")).toBeInTheDocument());
    fireEvent.contextMenu(screen.getByText("Empty"));
    fireEvent.click(screen.getByText("Löschen"));
    expect(api.folders.delete).toHaveBeenCalledWith("f1", "reparent");
    expect(screen.queryByText(/Was soll passieren/)).not.toBeInTheDocument();
  });

  it("shows the modal for a non-empty folder; 'Alles löschen' deletes recursively", async () => {
    mockFoldersLoad.mockResolvedValue([
      { id: "f1", name: "Full", parentId: null, position: 0, icon: "", color: "", sort: "manual", locked: false, mcpHidden: false },
    ]);
    mockLoad.mockResolvedValueOnce([
      { id: "n1", updatedAt: 1, pinned: false, archived: false, color: "", dueAt: null, folderId: "f1", position: 0, deletedAt: null, preview: "In folder", tasksDone: 0, tasksTotal: 0, protected: false, title: "", mcpHidden: false },
    ]);
    render(<App />);
    await waitFor(() => expect(screen.getByText("Full")).toBeInTheDocument());
    fireEvent.contextMenu(screen.getByText("Full"));
    fireEvent.click(screen.getByText("Löschen"));

    expect(await screen.findByText(/Enthält 1 Notiz/)).toBeInTheDocument();
    fireEvent.click(screen.getByText("Alles löschen"));
    expect(api.folders.delete).toHaveBeenCalledWith("f1", "recursive");
  });

  it("cancelling the folder-delete modal does not delete", async () => {
    mockFoldersLoad.mockResolvedValue([
      { id: "f1", name: "Full", parentId: null, position: 0, icon: "", color: "", sort: "manual", locked: false, mcpHidden: false },
    ]);
    mockLoad.mockResolvedValueOnce([
      { id: "n1", updatedAt: 1, pinned: false, archived: false, color: "", dueAt: null, folderId: "f1", position: 0, deletedAt: null, preview: "In folder", tasksDone: 0, tasksTotal: 0, protected: false, title: "", mcpHidden: false },
    ]);
    render(<App />);
    await waitFor(() => expect(screen.getByText("Full")).toBeInTheDocument());
    fireEvent.contextMenu(screen.getByText("Full"));
    fireEvent.click(screen.getByText("Löschen"));
    expect(await screen.findByText(/Enthält 1 Notiz/)).toBeInTheDocument();
    fireEvent.click(screen.getByText("Abbrechen"));
    expect(api.folders.delete).not.toHaveBeenCalled();
  });
});

describe("App — vault dialogs complete the pending protect action", () => {
  it("completing VaultSetup (setup -> save recovery key) applies the pending protect", async () => {
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.click(screen.getByTitle("Neue Notiz"));
    await waitFor(() => expect(screen.getByText("Ohne Titel")).toBeInTheDocument());
    fireEvent.contextMenu(screen.getByText("Ohne Titel"));
    fireEvent.click(screen.getByText("Notiz sperren"));
    await waitFor(() => expect(screen.getByText("Tresor einrichten")).toBeInTheDocument());

    fireEvent.change(screen.getByPlaceholderText("Passwort"), { target: { value: "secret123" } });
    fireEvent.change(screen.getByPlaceholderText("Passwort bestätigen"), { target: { value: "secret123" } });
    fireEvent.click(screen.getByText("Einrichten"));

    expect(await screen.findByText("Wiederherstellungs-Schlüssel")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Ich habe ihn gespeichert"));

    await waitFor(() => expect(api.vault.protectNote).toHaveBeenCalledWith(expect.any(String), true));
    expect(screen.queryByText("Wiederherstellungs-Schlüssel")).not.toBeInTheDocument();
  });

  // F5: the workspace already has a vault (another device seeded it). Setting
  // up a second one would mint an incompatible DEK, so the user is moved to
  // the unlock dialog — and the protect they started must survive the switch.
  it("switches setup to unlock when the workspace already has a vault, keeping the pending protect", async () => {
    vi.mocked(api.vault.setup).mockRejectedValueOnce(
      new Error("vault: already set up on the server — unlock with your passphrase"),
    );
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.click(screen.getByTitle("Neue Notiz"));
    await waitFor(() => expect(screen.getByText("Ohne Titel")).toBeInTheDocument());
    fireEvent.contextMenu(screen.getByText("Ohne Titel"));
    fireEvent.click(screen.getByText("Notiz sperren"));
    await waitFor(() => expect(screen.getByText("Tresor einrichten")).toBeInTheDocument());

    fireEvent.change(screen.getByPlaceholderText("Passwort"), { target: { value: "secret123" } });
    fireEvent.change(screen.getByPlaceholderText("Passwort bestätigen"), { target: { value: "secret123" } });
    fireEvent.click(screen.getByText("Einrichten"));

    // The setup dialog is replaced by the unlock one...
    expect(await screen.findByText("Tresor entsperren")).toBeInTheDocument();
    expect(screen.queryByText("Tresor einrichten")).not.toBeInTheDocument();

    // ...and the protect the user started still happens once they are in.
    fireEvent.change(screen.getByPlaceholderText("Passwort"), { target: { value: "secret123" } });
    fireEvent.click(screen.getByText("Entsperren"));
    await waitFor(() => expect(api.vault.protectNote).toHaveBeenCalledWith(expect.any(String), true));
  });

  // R2: a device whose ring is behind the workspace's generation is refused
  // by the backend. Saying nothing would leave the note looking unchanged.
  it("explains a refused protect instead of failing silently", async () => {
    vi.mocked(api.vault.status).mockResolvedValue(vaultStatus({ exists: true, unlocked: true }));
    vi.mocked(api.vault.protectNote).mockRejectedValueOnce(
      new Error("vault: key generation outdated — unlock with your passphrase"),
    );
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.click(screen.getByTitle("Neue Notiz"));
    await waitFor(() => expect(screen.getByText("Ohne Titel")).toBeInTheDocument());
    fireEvent.contextMenu(screen.getByText("Ohne Titel"));
    fireEvent.click(screen.getByText("Notiz sperren"));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Dieses Gerät hat den neuesten Tresorschlüssel noch nicht.",
    );
    expect(screen.getByText("Schützen nicht möglich")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Schließen"));
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  });

  it("cancelling VaultSetup drops the pending protect", async () => {
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.click(screen.getByTitle("Neue Notiz"));
    await waitFor(() => expect(screen.getByText("Ohne Titel")).toBeInTheDocument());
    fireEvent.contextMenu(screen.getByText("Ohne Titel"));
    fireEvent.click(screen.getByText("Notiz sperren"));
    await waitFor(() => expect(screen.getByText("Tresor einrichten")).toBeInTheDocument());
    fireEvent.click(screen.getByText("Abbrechen"));
    expect(screen.queryByText("Tresor einrichten")).not.toBeInTheDocument();
    expect(api.vault.protectNote).not.toHaveBeenCalled();
  });

  it("routes protect through VaultUnlock when the vault exists but is locked, and completes it after unlocking", async () => {
    vi.mocked(api.vault.status).mockResolvedValue(vaultStatus({ exists: true, unlocked: false }));
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.click(screen.getByTitle("Neue Notiz"));
    await waitFor(() => expect(screen.getByText("Ohne Titel")).toBeInTheDocument());
    fireEvent.contextMenu(screen.getByText("Ohne Titel"));
    fireEvent.click(screen.getByText("Notiz sperren"));
    await waitFor(() => expect(screen.getByText("Tresor entsperren")).toBeInTheDocument());

    fireEvent.change(screen.getByPlaceholderText("Passwort"), { target: { value: "secret123" } });
    fireEvent.click(screen.getByText("Entsperren"));

    await waitFor(() => expect(api.vault.protectNote).toHaveBeenCalledWith(expect.any(String), true));
  });

  it("cancelling VaultUnlock drops the pending protect", async () => {
    vi.mocked(api.vault.status).mockResolvedValue(vaultStatus({ exists: true, unlocked: false }));
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.click(screen.getByTitle("Neue Notiz"));
    await waitFor(() => expect(screen.getByText("Ohne Titel")).toBeInTheDocument());
    fireEvent.contextMenu(screen.getByText("Ohne Titel"));
    fireEvent.click(screen.getByText("Notiz sperren"));
    await waitFor(() => expect(screen.getByText("Tresor entsperren")).toBeInTheDocument());
    fireEvent.click(screen.getByText("Abbrechen"));
    expect(screen.queryByText("Tresor entsperren")).not.toBeInTheDocument();
    expect(api.vault.protectNote).not.toHaveBeenCalled();
  });
});

describe("App — rotation code after a Touch ID unlock", () => {
  // Touch ID types no passphrase, so the unlock dialog cannot redeem the
  // waiting code itself — App has to take that step over, or the member's
  // re-sealed notes stay unreadable with no way back in.
  const locked = vaultStatus({ exists: true, biometric: true, rotationCode: true });

  const unlockViaTouchId = async () => {
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.click(screen.getByTitle("Neue Notiz"));
    await waitFor(() => expect(screen.getByText("Ohne Titel")).toBeInTheDocument());
    fireEvent.contextMenu(screen.getByText("Ohne Titel"));
    fireEvent.click(screen.getByText("Notiz sperren"));
    await waitFor(() => expect(api.vault.unlockBiometric).toHaveBeenCalled());
  };

  it("prompts for the code and redeems it with the passphrase", async () => {
    vi.mocked(api.vault.status).mockResolvedValue(locked);
    await unlockViaTouchId();

    expect(await screen.findByText("Wechsel-Code eingeben")).toBeInTheDocument();
    fireEvent.change(screen.getByPlaceholderText("Wechsel-Code"), { target: { value: "AAAA-BBBB" } });
    fireEvent.change(screen.getByPlaceholderText("Passwort"), { target: { value: "member-pw" } });
    fireEvent.click(screen.getByText("Schlüssel wechseln"));

    await waitFor(() => expect(api.vault.rotationRedeem).toHaveBeenCalledWith("AAAA-BBBB", "member-pw"));
    await waitFor(() => expect(screen.queryByText("Wechsel-Code eingeben")).not.toBeInTheDocument());
  });

  it("does not prompt when no code is waiting", async () => {
    vi.mocked(api.vault.status).mockResolvedValue({ ...locked, rotationCode: false });
    await unlockViaTouchId();

    await waitFor(() => expect(api.vault.protectNote).toHaveBeenCalledWith(expect.any(String), true));
    expect(screen.queryByText("Wechsel-Code eingeben")).not.toBeInTheDocument();
  });
});

describe("App — images stay unencrypted hint", () => {
  const withImage = '<p>hi <img src="notefix-img://a.png"> there</p>';
  beforeEach(() => localStorage.clear());
  afterEach(() => localStorage.clear());

  /** Create a note and pick "Notiz sperren" from its context menu. */
  const lockFirstNote = async () => {
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    fireEvent.click(screen.getByTitle("Neue Notiz"));
    await waitFor(() => expect(screen.getByText("Ohne Titel")).toBeInTheDocument());
    fireEvent.contextMenu(screen.getByText("Ohne Titel"));
    fireEvent.click(screen.getByText("Notiz sperren"));
  };

  it("warns once before protecting a note that embeds an image, then proceeds", async () => {
    vi.mocked(api.notes.loadOne).mockResolvedValue(withImage);
    await lockFirstNote();

    expect(await screen.findByText("Bilder bleiben unverschlüsselt")).toBeInTheDocument();
    // Nothing has been protected yet — the vault gate has not even been reached.
    expect(api.vault.protectNote).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Notiz sperren" }));
    await waitFor(() => expect(screen.getByText("Tresor einrichten")).toBeInTheDocument());
    expect(localStorage.getItem("vault.imagesHintSeen")).toBe("1");
  });

  it("does not warn again once the hint has been acknowledged", async () => {
    localStorage.setItem("vault.imagesHintSeen", "1");
    vi.mocked(api.notes.loadOne).mockResolvedValue(withImage);
    await lockFirstNote();

    await waitFor(() => expect(screen.getByText("Tresor einrichten")).toBeInTheDocument());
    expect(screen.queryByText("Bilder bleiben unverschlüsselt")).not.toBeInTheDocument();
  });

  it("does not warn for a note without images", async () => {
    vi.mocked(api.notes.loadOne).mockResolvedValue("<p>just text</p>");
    await lockFirstNote();

    await waitFor(() => expect(screen.getByText("Tresor einrichten")).toBeInTheDocument());
    expect(screen.queryByText("Bilder bleiben unverschlüsselt")).not.toBeInTheDocument();
  });

  it("cancelling the hint drops the protect and keeps the hint for next time", async () => {
    vi.mocked(api.notes.loadOne).mockResolvedValue(withImage);
    await lockFirstNote();

    expect(await screen.findByText("Bilder bleiben unverschlüsselt")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Abbrechen" }));

    await waitFor(() => expect(screen.queryByText("Bilder bleiben unverschlüsselt")).not.toBeInTheDocument());
    expect(screen.queryByText("Tresor einrichten")).not.toBeInTheDocument();
    expect(api.vault.protectNote).not.toHaveBeenCalled();
    expect(localStorage.getItem("vault.imagesHintSeen")).toBeNull();
  });
});

describe("App — perNote reveal flow", () => {
  it("unlocking from the locked-note placeholder reveals just that note (perNote scope)", async () => {
    // Persistent (not just the initial call): the dialog's own unlock() calls
    // vault.refresh(), which re-queries status — it must keep reporting
    // unlocked, or the placeholder never lets go.
    vi.mocked(api.vault.status).mockResolvedValue({ exists: true, unlocked: true, biometric: false });
    vi.mocked(api.settings.load).mockResolvedValueOnce({ vaultLockScope: "perNote" });
    mockLoad.mockResolvedValueOnce([{
      id: "n1", updatedAt: Date.now(), pinned: false, archived: false, color: "", dueAt: null, folderId: null,
      position: 0, deletedAt: null, preview: "", tasksDone: 0, tasksTotal: 0, protected: true, title: "", mcpHidden: false,
    }]);

    render(<App />);

    await waitFor(() => expect(screen.getByText("Diese Notiz ist geschützt")).toBeInTheDocument());
    fireEvent.click(screen.getByText("Entsperren"));
    await waitFor(() => expect(screen.getByText("Tresor entsperren")).toBeInTheDocument());

    // The locked placeholder (with its own "Entsperren" button) stays mounted
    // behind the VaultUnlock modal, so there are now two matches — the dialog's
    // submit button is the last one in document order.
    fireEvent.change(screen.getByPlaceholderText("Passwort"), { target: { value: "secret123" } });
    fireEvent.click(screen.getAllByText("Entsperren").at(-1)!);

    await waitFor(() => expect(api.notes.loadOne).toHaveBeenCalled());
    expect(screen.queryByText("Diese Notiz ist geschützt")).not.toBeInTheDocument();
  });
});

describe("App — auto-lock on hide/sleep", () => {
  const setHidden = (hidden: boolean) => Object.defineProperty(document, "hidden", { value: hidden, configurable: true });
  afterEach(() => setHidden(false));

  it("locks the vault when the document is hidden and autoLockOnHide is on (autoLockOnSleep off)", async () => {
    vi.mocked(api.vault.status).mockResolvedValue(vaultStatus({ exists: true, unlocked: true }));
    vi.mocked(api.settings.load).mockResolvedValueOnce({ autoLockOnHide: "true", autoLockOnSleep: "false" });
    render(<App />);
    // Wait for the vault-status refresh (async) to land before triggering the
    // event, same race the "lock vault shortcut" test above documents.
    await waitFor(() => expect(screen.getByTitle("Jetzt sperren")).toBeInTheDocument());
    setHidden(true);
    fireEvent(document, new Event("visibilitychange"));
    await waitFor(() => expect(api.vault.lock).toHaveBeenCalled());
  });

  it("locks the vault when the document is hidden and autoLockOnSleep is on (autoLockOnHide off)", async () => {
    vi.mocked(api.vault.status).mockResolvedValue(vaultStatus({ exists: true, unlocked: true }));
    vi.mocked(api.settings.load).mockResolvedValueOnce({ autoLockOnHide: "false", autoLockOnSleep: "true" });
    render(<App />);
    await waitFor(() => expect(screen.getByTitle("Jetzt sperren")).toBeInTheDocument());
    setHidden(true);
    fireEvent(document, new Event("visibilitychange"));
    await waitFor(() => expect(api.vault.lock).toHaveBeenCalled());
  });

  it("does not lock when both autoLockOnHide and autoLockOnSleep are off", async () => {
    vi.mocked(api.vault.status).mockResolvedValue(vaultStatus({ exists: true, unlocked: true }));
    vi.mocked(api.settings.load).mockResolvedValueOnce({ autoLockOnHide: "false", autoLockOnSleep: "false" });
    render(<App />);
    await waitFor(() => expect(screen.getByTitle("Jetzt sperren")).toBeInTheDocument());
    setHidden(true);
    fireEvent(document, new Event("visibilitychange"));
    expect(api.vault.lock).not.toHaveBeenCalled();
  });
});

describe("App — MCP config apply", () => {
  it("skips mcpApplyConfig on initial mount, but applies it for a user-driven settings change", async () => {
    render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    expect(api.mcpApplyConfig).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTitle("Mehr"));
    fireEvent.click(screen.getByText("Einstellungen"));
    fireEvent.click(screen.getByText("MCP"));
    fireEvent.click(screen.getByRole("switch", { name: "Server aktiv" }));

    await waitFor(() => expect(api.mcpApplyConfig).toHaveBeenCalledWith(expect.objectContaining({ enabled: true })));
  });
});

describe("App — dashboard startView", () => {
  it("opens directly to the dashboard when settings.startView is 'dashboard'", async () => {
    vi.mocked(api.settings.load).mockResolvedValueOnce({ startView: "dashboard" });
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Dashboard" })).toBeInTheDocument();
  });
});

describe("App — sidebar side and mode", () => {
  it("sidebarSide 'right' reverses the split layout", async () => {
    vi.mocked(api.settings.load).mockResolvedValueOnce({ sidebarSide: "right" });
    const { container } = render(<App />);
    await waitFor(() => screen.getByTitle("Neue Notiz"));
    expect(container.querySelector(".flex-row-reverse")).toBeTruthy();
  });

  it("sidebarMode 'combined' renders the combined note list instead", async () => {
    vi.mocked(api.settings.load).mockResolvedValueOnce({ sidebarMode: "combined" });
    render(<App />);
    await waitFor(() => expect(screen.getByText("Alle Kontexte")).toBeInTheDocument());
  });
});

describe("App — mobile layout", () => {
  const setMobile = (matches: boolean) => {
    window.matchMedia = vi.fn().mockImplementation((query: string) => ({
      matches, media: query, addEventListener: vi.fn(), removeEventListener: vi.fn(),
    })) as unknown as typeof window.matchMedia;
  };
  afterEach(() => { (window as unknown as { matchMedia?: unknown }).matchMedia = undefined; });

  it("shows the list first; selecting a note shows the editor with a working back button", async () => {
    setMobile(true);
    mockLoad.mockResolvedValueOnce([{
      id: "a", updatedAt: 1, pinned: false, archived: false, color: "", dueAt: null, folderId: null,
      position: 0, deletedAt: null, preview: "Mobile note", tasksDone: 0, tasksTotal: 0, protected: false, title: "", mcpHidden: false,
    }]);
    render(<App />);

    await waitFor(() => expect(screen.getByText("Mobile note")).toBeInTheDocument());
    expect(screen.queryByText("Notizen")).not.toBeInTheDocument();

    fireEvent.click(screen.getByText("Mobile note"));
    await waitFor(() => expect(screen.getByText("Notizen")).toBeInTheDocument());
    expect(screen.queryByText("Mobile note")).not.toBeInTheDocument();

    fireEvent.click(screen.getByText("Notizen"));
    await waitFor(() => expect(screen.getByText("Mobile note")).toBeInTheDocument());
    expect(screen.queryByText("Notizen")).not.toBeInTheDocument();
  });
});
