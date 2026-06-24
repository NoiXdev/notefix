import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import type { Note } from "../types";

vi.mock("@tiptap/react", () => {
  // Return a STABLE editor object so NoteEditor's `[note.id, editor]` effect
  // doesn't re-run every render (an unstable mock + setProgress => infinite loop).
  const editor = {
    isActive: () => false,
    chain: () => ({
      focus: () => ({
        toggleBold: () => ({ run: vi.fn() }),
        toggleItalic: () => ({ run: vi.fn() }),
        toggleUnderline: () => ({ run: vi.fn() }),
        toggleStrike: () => ({ run: vi.fn() }),
        toggleBulletList: () => ({ run: vi.fn() }),
        toggleOrderedList: () => ({ run: vi.fn() }),
        toggleTaskList: () => ({ run: vi.fn() }),
        setImage: () => ({ run: vi.fn() }),
      }),
    }),
    commands: { setContent: vi.fn(), focus: vi.fn() },
    getHTML: () => "<p></p>",
    isEditable: true,
    on: vi.fn(),
    off: vi.fn(),
    state: { selection: { from: 0, to: 0 }, doc: { textContent: "" } },
  };
  return { useEditor: () => editor, EditorContent: () => null };
});
vi.mock("@tiptap/starter-kit", () => ({ default: {} }));
vi.mock("@tiptap/extension-underline", () => ({ default: {} }));
vi.mock("@tiptap/extension-placeholder", () => ({ default: { configure: () => ({}) } }));
vi.mock("@tiptap/extension-task-list", () => ({ default: {} }));
vi.mock("@tiptap/extension-task-item", () => ({ default: { configure: () => ({}) } }));
vi.mock("./ResizableImage", () => ({
  ResizableImage: { configure: () => ({}) },
}));

const { mockToggleAlwaysOnTop, mockCloseWindow } = vi.hoisted(() => ({
  mockToggleAlwaysOnTop: vi.fn<(current: boolean) => Promise<boolean>>(),
  mockCloseWindow: vi.fn<() => Promise<void>>(),
}));

vi.mock("../api", () => ({
  api: {
    notes: { load: vi.fn(), save: vi.fn(), delete: vi.fn() },
    onNotesChanged: () => () => {},
    openNoteWindow: vi.fn(),
    setWindowTitle: vi.fn(),
    toggleAlwaysOnTop: mockToggleAlwaysOnTop,
    closeWindow: mockCloseWindow,
    getAppInfo: vi.fn(),
    openExternal: vi.fn(),
  },
}));

const { default: NoteEditor } = await import("./NoteEditor");

const mockNote: Note = { id: "1", content: "<p>Hello</p>", updatedAt: 1000, pinned: false, archived: false, color: "", dueAt: null };
const onChange = vi.fn();

beforeEach(() => {
  vi.clearAllMocks();
  mockToggleAlwaysOnTop.mockResolvedValue(true);
});

describe("NoteEditor — main window mode (isWindow=false)", () => {
  it("renders the formatting toolbar", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    expect(screen.getByTitle("Fett")).toBeInTheDocument();
    expect(screen.getByTitle("Kursiv")).toBeInTheDocument();
    expect(screen.getByTitle("Unterstrichen")).toBeInTheDocument();
    expect(screen.getByTitle("Durchgestrichen")).toBeInTheDocument();
    expect(screen.getByTitle("Aufzählung")).toBeInTheDocument();
    expect(screen.getByTitle("Nummerierte Liste")).toBeInTheDocument();
  });

  it('shows the "open in new window" button', () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    expect(screen.getByTitle("In neuem Fenster öffnen")).toBeInTheDocument();
  });

  it("does not show the custom title bar", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    expect(screen.queryByTitle("Im Vordergrund halten")).not.toBeInTheDocument();
    expect(screen.queryByTitle("Schließen")).not.toBeInTheDocument();
  });

  it("renders the save indicator", () => {
    render(<NoteEditor note={{ id: 'a', content: '<p>x</p>', updatedAt: 1, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0, deletedAt: null }} onChange={vi.fn()} />);
    expect(screen.getByLabelText('Speichern')).toBeInTheDocument();
  });

  it("shows a status bar with word/character counts", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    expect(screen.getByText(/Wörter:/)).toBeInTheDocument();
  });
});

describe("NoteEditor — standalone window mode (isWindow=true)", () => {
  it("shows the custom title bar with pin and close buttons", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} isWindow />);
    expect(screen.getByTitle("Im Vordergrund halten")).toBeInTheDocument();
    expect(screen.getByTitle("Schließen")).toBeInTheDocument();
  });

  it('does not show the "open in new window" button', () => {
    render(<NoteEditor note={mockNote} onChange={onChange} isWindow />);
    expect(screen.queryByTitle("In neuem Fenster öffnen")).not.toBeInTheDocument();
  });

  it("toggles pin label when api.toggleAlwaysOnTop resolves true", async () => {
    render(<NoteEditor note={mockNote} onChange={onChange} isWindow />);
    const pinBtn = screen.getByTitle("Im Vordergrund halten");
    fireEvent.click(pinBtn);
    await screen.findByTitle("Nicht mehr anheften");
    expect(mockToggleAlwaysOnTop).toHaveBeenCalledWith(false);
  });

  it("closes the window via the Tauri API (not DOM window.close)", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} isWindow />);
    fireEvent.click(screen.getByTitle("Schließen"));
    expect(mockCloseWindow).toHaveBeenCalledOnce();
  });
});

describe("NoteEditor — task list", () => {
  it("shows the task-list toolbar button", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    expect(screen.getByTitle("Aufgabenliste")).toBeInTheDocument();
  });
});

describe("NoteEditor — due date", () => {
  it("renders the due-date control and reports changes", () => {
    const onSetDue = vi.fn();
    render(<NoteEditor note={mockNote} onChange={onChange} onSetDue={onSetDue} />);
    const input = screen.getByLabelText("Fälligkeitsdatum");
    fireEvent.change(input, { target: { value: "2026-06-23" } });
    expect(onSetDue).toHaveBeenCalledWith(mockNote.id, new Date(2026, 5, 23).getTime());
  });
});
