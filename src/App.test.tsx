import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, beforeEach, vi } from "vitest";

vi.mock("@tiptap/react", () => {
  // Return a STABLE editor object so NoteEditor's `[note.id, editor]` effect
  // doesn't re-run every render (an unstable mock + setProgress => infinite loop).
  const editor = {
    isActive: () => false,
    chain: () => ({ focus: () => ({ toggleBold: () => ({ run: vi.fn() }) }) }),
    commands: { setContent: vi.fn(), focus: vi.fn() },
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
    notes: { load: mockLoad, save: mockSave, delete: mockDeleteFn, setPinned: mockSetPinned, setArchived: vi.fn(), setColor: vi.fn(), setDue: vi.fn(), setFolder: vi.fn(), reorder: vi.fn(), restore: vi.fn(() => Promise.resolve()), purge: vi.fn(() => Promise.resolve()) },
    trash: { load: vi.fn(() => Promise.resolve([])), empty: vi.fn(() => Promise.resolve()) },
    folders: { load: () => Promise.resolve([]), create: vi.fn(), rename: vi.fn(), move: vi.fn(), delete: vi.fn(), reorder: vi.fn() },
    exportNotes: vi.fn(),
    stats: vi.fn(() => Promise.resolve({ notes: 0, archived: 0, characters: 0, words: 0 })),
    settings: { load: () => Promise.resolve({}), set: vi.fn() },
    autostart: { isEnabled: () => Promise.resolve(false), enable: vi.fn(), disable: vi.fn() },
    checkPaths: vi.fn(() => Promise.resolve({ dbWritable: true, imagesWritable: true, dbPath: '', imagesPath: '' })),
    windowProbe: vi.fn(() => Promise.resolve(true)),
    onTrayEvent: () => () => {},
    onNotesChanged: () => () => {},
    onCloseRequested: () => () => {},
    quitApp: vi.fn(),
    hideMain: vi.fn(),
    openNoteWindow: vi.fn(),
    setWindowTitle: vi.fn(),
    toggleAlwaysOnTop: vi.fn(),
    closeWindow: vi.fn(),
    getAppInfo: vi.fn(),
    openExternal: vi.fn(),
    mcpApplyConfig: vi.fn(() => Promise.resolve()),
  },
}));

import App from "./App";

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
    await waitFor(() => expect(screen.getByText("New note")).toBeInTheDocument());
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
