import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import type { Note } from "../types";

vi.mock("@tiptap/react", () => ({
  useEditor: () => ({
    isActive: () => false,
    chain: () => ({
      focus: () => ({
        toggleBold: () => ({ run: vi.fn() }),
        toggleItalic: () => ({ run: vi.fn() }),
        toggleUnderline: () => ({ run: vi.fn() }),
        toggleStrike: () => ({ run: vi.fn() }),
        toggleBulletList: () => ({ run: vi.fn() }),
        toggleOrderedList: () => ({ run: vi.fn() }),
        setImage: () => ({ run: vi.fn() }),
      }),
    }),
    commands: { setContent: vi.fn(), focus: vi.fn() },
    getHTML: () => "<p></p>",
    isEditable: true,
  }),
  EditorContent: () => null,
}));
vi.mock("@tiptap/starter-kit", () => ({ default: {} }));
vi.mock("@tiptap/extension-underline", () => ({ default: {} }));
vi.mock("@tiptap/extension-placeholder", () => ({ default: { configure: () => ({}) } }));
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

const mockNote: Note = { id: "1", content: "<p>Hello</p>", updatedAt: 1000 };
const onChange = vi.fn();

beforeEach(() => {
  vi.clearAllMocks();
  mockToggleAlwaysOnTop.mockResolvedValue(true);
});

describe("NoteEditor — main window mode (isWindow=false)", () => {
  it("renders the formatting toolbar", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    expect(screen.getByTitle("Bold")).toBeInTheDocument();
    expect(screen.getByTitle("Italic")).toBeInTheDocument();
    expect(screen.getByTitle("Underline")).toBeInTheDocument();
    expect(screen.getByTitle("Strikethrough")).toBeInTheDocument();
    expect(screen.getByTitle("Bullet list")).toBeInTheDocument();
    expect(screen.getByTitle("Numbered list")).toBeInTheDocument();
  });

  it('shows the "open in new window" button', () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    expect(screen.getByTitle("Open in new window")).toBeInTheDocument();
  });

  it("does not show the custom title bar", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    expect(screen.queryByTitle("Keep on top")).not.toBeInTheDocument();
    expect(screen.queryByTitle("Close")).not.toBeInTheDocument();
  });
});

describe("NoteEditor — standalone window mode (isWindow=true)", () => {
  it("shows the custom title bar with pin and close buttons", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} isWindow />);
    expect(screen.getByTitle("Keep on top")).toBeInTheDocument();
    expect(screen.getByTitle("Close")).toBeInTheDocument();
  });

  it('does not show the "open in new window" button', () => {
    render(<NoteEditor note={mockNote} onChange={onChange} isWindow />);
    expect(screen.queryByTitle("Open in new window")).not.toBeInTheDocument();
  });

  it("toggles pin label when api.toggleAlwaysOnTop resolves true", async () => {
    render(<NoteEditor note={mockNote} onChange={onChange} isWindow />);
    const pinBtn = screen.getByTitle("Keep on top");
    fireEvent.click(pinBtn);
    await screen.findByTitle("Unpin window");
    expect(mockToggleAlwaysOnTop).toHaveBeenCalledWith(false);
  });

  it("closes the window via the Tauri API (not DOM window.close)", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} isWindow />);
    fireEvent.click(screen.getByTitle("Close"));
    expect(mockCloseWindow).toHaveBeenCalledOnce();
  });
});
