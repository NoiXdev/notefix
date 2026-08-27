import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, beforeEach, vi } from "vitest";

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
vi.mock("./export", () => ({ exportSelected: vi.fn() }));

const { mockLoad, mockSave, mockDeleteFn, mockSetPinned } = vi.hoisted(() => ({
  mockLoad: vi.fn(() => Promise.resolve([] as unknown[])),
  mockSave: vi.fn(() => Promise.resolve(undefined)),
  mockDeleteFn: vi.fn(() => Promise.resolve(undefined)),
  mockSetPinned: vi.fn(() => Promise.resolve(undefined)),
}));

vi.mock("./api", () => ({
  api: {
    notes: { load: mockLoad, loadOne: vi.fn(() => Promise.resolve('<p></p>')), search: vi.fn(() => Promise.resolve([])), searchAll: vi.fn(() => Promise.resolve([])), save: mockSave, delete: mockDeleteFn, setPinned: mockSetPinned, setArchived: vi.fn(), setColor: vi.fn(), setDue: vi.fn(), setFolder: vi.fn(), reorder: vi.fn(), restore: vi.fn(() => Promise.resolve()), purge: vi.fn(() => Promise.resolve()) },
    trash: { load: vi.fn(() => Promise.resolve([])), empty: vi.fn(() => Promise.resolve()) },
    folders: { load: () => Promise.resolve([]), create: vi.fn(), rename: vi.fn(), move: vi.fn(), delete: vi.fn(), reorder: vi.fn() },
    exportNotes: vi.fn(),
    stats: vi.fn(() => Promise.resolve({ notes: 0, archived: 0, characters: 0, words: 0 })),
    settings: { load: vi.fn(() => Promise.resolve({})), set: vi.fn() },
    autostart: { isEnabled: () => Promise.resolve(false), enable: vi.fn(), disable: vi.fn() },
    checkPaths: vi.fn(() => Promise.resolve({ dbWritable: true, imagesWritable: true, dbPath: '', imagesPath: '' })),
    windowProbe: vi.fn(() => Promise.resolve(true)),
    onTrayEvent: () => () => {},
    onNotesChanged: () => () => {},
    onCloseRequested: () => () => {},
    onContextChanged: () => () => {},
    onAuthCallback: () => () => {},
    onSyncStatus: () => () => {},
    contexts: {
      list: () => Promise.resolve([]),
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
    getAppInfo: vi.fn(),
    openExternal: vi.fn(),
    mcpApplyConfig: vi.fn(() => Promise.resolve()),
    vault: {
      status: vi.fn(() => Promise.resolve({ exists: false, unlocked: false, biometric: false })),
      setup: vi.fn(() => Promise.resolve([])),
      unlock: vi.fn(() => Promise.resolve()),
      unlockRecovery: vi.fn(() => Promise.resolve()),
      unlockBiometric: vi.fn(() => Promise.resolve()),
      lock: vi.fn(() => Promise.resolve()),
      changePassphrase: vi.fn(() => Promise.resolve()),
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

beforeEach(() => {
  vi.clearAllMocks();
  mockLoad.mockResolvedValue([]);
  mockSave.mockResolvedValue(undefined);
  mockDeleteFn.mockResolvedValue(undefined);
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
    vi.mocked(api.vault.status).mockResolvedValueOnce({ exists: true, unlocked: false, biometric: false });
    mockLoad.mockResolvedValueOnce([protectedNoteMeta]);

    render(<App />);

    await waitFor(() => expect(screen.getByText("Diese Notiz ist geschützt")).toBeInTheDocument());
    expect(api.notes.loadOne).not.toHaveBeenCalled();

    fireEvent.click(screen.getByText("Entsperren"));
    await waitFor(() => expect(screen.getByText("Tresor entsperren")).toBeInTheDocument());
  });
});

describe("App — auto-lock idle timer", () => {
  it("locks the vault after autoLockMinutes of inactivity", async () => {
    vi.mocked(api.vault.status).mockResolvedValueOnce({ exists: true, unlocked: true, biometric: false });
    vi.mocked(api.settings.load).mockResolvedValueOnce({ autoLockMode: "after", autoLockMinutes: "0.01" });

    render(<App />);

    await waitFor(() => expect(screen.getByTitle("Neue Notiz")).toBeInTheDocument());
    await waitFor(() => expect(api.vault.lock).toHaveBeenCalled(), { timeout: 3000 });
  });
});
