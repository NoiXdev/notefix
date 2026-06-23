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
vi.mock("@tiptap/starter-kit", () => ({ default: {} }));
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
    notes: { load: mockLoad, save: mockSave, delete: mockDeleteFn, setPinned: mockSetPinned, setArchived: vi.fn(), setColor: vi.fn() },
    exportNotes: vi.fn(),
    settings: { load: () => Promise.resolve({}), set: vi.fn() },
    autostart: { isEnabled: () => Promise.resolve(false), enable: vi.fn(), disable: vi.fn() },
    onTrayEvent: () => () => {},
    onNotesChanged: () => () => {},
    openNoteWindow: vi.fn(),
    setWindowTitle: vi.fn(),
    toggleAlwaysOnTop: vi.fn(),
    closeWindow: vi.fn(),
    getAppInfo: vi.fn(),
    openExternal: vi.fn(),
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
    await waitFor(() => expect(screen.getByText(/select a note/i)).toBeInTheDocument());
  });

  it("renders the sidebar", async () => {
    render(<App />);
    await waitFor(() => {
      expect(screen.getByText("Notes")).toBeInTheDocument();
      expect(screen.getByTitle("New note")).toBeInTheDocument();
    });
  });
});

describe("App — creating notes", () => {
  it("creates a note and auto-selects it when + is clicked", async () => {
    render(<App />);
    await waitFor(() => screen.getByTitle("New note"));
    fireEvent.click(screen.getByTitle("New note"));
    await waitFor(() => expect(screen.getByTitle("Bold")).toBeInTheDocument());
    expect(screen.queryByText(/select a note/i)).not.toBeInTheDocument();
  });

  it("shows the new note in the sidebar list", async () => {
    render(<App />);
    await waitFor(() => screen.getByTitle("New note"));
    fireEvent.click(screen.getByTitle("New note"));
    await waitFor(() => expect(screen.getByText("New note")).toBeInTheDocument());
  });
});

describe("App — deleting notes", () => {
  it("returns to empty state when the only note is deleted", async () => {
    render(<App />);
    await waitFor(() => screen.getByTitle("New note"));
    fireEvent.click(screen.getByTitle("New note"));
    await waitFor(() => screen.getByTitle("Delete note"));
    fireEvent.click(screen.getByTitle("Delete note"));
    await waitFor(() => expect(screen.getByText(/select a note/i)).toBeInTheDocument());
  });
});
