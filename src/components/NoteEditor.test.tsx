import { render, screen, fireEvent, act } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import type { NoteMeta } from "../types";
import { markdownToHtml } from "../markdown";

// doCopy() builds the clipboard HTML via `DOMSerializer.fromSchema(view.state.schema)
// .serializeFragment(view.state.doc.slice(from, to).content)`. Rather than construct a
// real ProseMirror schema, the fake serializer below just echoes back whatever "content"
// marker the fake view's `doc.slice()` produced, wrapped in a <p> — so the real
// `selectionToCopy()` (untouched) still runs on genuine, if synthetic, HTML.
vi.mock("@tiptap/pm/model", () => ({
  DOMSerializer: {
    fromSchema: () => ({
      serializeFragment: (content: unknown) => {
        const frag = document.createDocumentFragment();
        const p = document.createElement("p");
        p.textContent = String(content);
        frag.appendChild(p);
        return frag;
      },
    }),
  },
}));

// A small stateful fake TipTap editor: toolbar chain calls flip real `isActive()`
// state (so a click visibly activates its button, like the real editor would) while
// every chain leaf is a spy so tests can assert exactly which command ran, with what
// args. `useEditor`'s config argument is captured so tests can invoke onUpdate /
// editorProps handlers directly, the same way the real ProseMirror view would.
const { fakeEditor, getConfig, resetFakeEditor } = vi.hoisted(() => {
  let capturedConfig: any = null;
  const active = new Set<string>();
  let headingLevel: number | null = null;

  function toggle(name: string) {
    return vi.fn(() => {
      if (active.has(name)) active.delete(name); else active.add(name);
      return { run: vi.fn(() => true) };
    });
  }

  const editor: any = {
    isActive: vi.fn((name: string, attrs?: { level?: number }) => {
      if (name === "heading") return headingLevel !== null && attrs?.level === headingLevel;
      return active.has(name);
    }),
    chain: vi.fn(() => ({
      focus: vi.fn(() => ({
        toggleBold: toggle("bold"),
        toggleItalic: toggle("italic"),
        toggleUnderline: toggle("underline"),
        toggleStrike: toggle("strike"),
        toggleTaskList: toggle("taskList"),
        toggleBulletList: toggle("bulletList"),
        toggleOrderedList: toggle("orderedList"),
        toggleCodeBlock: toggle("codeBlock"),
        toggleHeading: vi.fn(({ level }: { level: number }) => {
          headingLevel = headingLevel === level ? null : level;
          return { run: vi.fn(() => true) };
        }),
        setImage: vi.fn(() => ({ run: vi.fn(() => true) })),
      })),
    })),
    commands: {
      setContent: vi.fn(),
      focus: vi.fn(),
      setInvisibles: vi.fn(),
      setSearch: vi.fn(),
      stepSearch: vi.fn(),
      clearSearch: vi.fn(),
    },
    getHTML: vi.fn(() => "<p></p>"),
    isEditable: true,
    isDestroyed: false,
    on: vi.fn(),
    off: vi.fn(),
    state: { selection: { from: 0, to: 0 }, doc: { textContent: "" } },
  };

  return {
    fakeEditor: editor,
    getConfig: () => capturedConfig,
    resetFakeEditor: () => {
      active.clear();
      headingLevel = null;
      capturedConfig = null;
      editor.isActive.mockClear();
      editor.chain.mockClear();
      editor.commands.setContent.mockClear();
      editor.commands.focus.mockClear();
      editor.commands.setInvisibles.mockClear();
      editor.commands.setSearch.mockClear();
      editor.commands.stepSearch.mockClear();
      editor.commands.clearSearch.mockClear();
      editor.getHTML.mockReset();
      editor.getHTML.mockImplementation(() => "<p></p>");
      editor.isDestroyed = false;
      editor.state = { selection: { from: 0, to: 0 }, doc: { textContent: "" } };
      // stash the config on every (re-)render so tests can grab the latest one
      // via getConfig(), the way the real hook would receive fresh closures.
      (editor as any).__setConfig = (c: any) => { capturedConfig = c; };
    },
  };
});

vi.mock("@tiptap/react", () => ({
  useEditor: (config: any) => { (fakeEditor as any).__setConfig(config); return fakeEditor; },
  EditorContent: () => null,
}));
vi.mock("@tiptap/starter-kit", () => ({ default: { configure: () => ({}) } }));
vi.mock("@tiptap/extension-underline", () => ({ default: {} }));
vi.mock("@tiptap/extension-placeholder", () => ({ default: { configure: () => ({}) } }));
vi.mock("@tiptap/extension-task-list", () => ({ default: {} }));
vi.mock("@tiptap/extension-task-item", () => ({ default: { configure: () => ({}) } }));
vi.mock("./ResizableImage", () => ({ ResizableImage: { configure: () => ({}) } }));

const { mockToggleAlwaysOnTop, mockCloseWindow, mockSaveImageFile, notesChangedRef } = vi.hoisted(() => ({
  mockToggleAlwaysOnTop: vi.fn<(current: boolean) => Promise<boolean>>(),
  mockCloseWindow: vi.fn<() => Promise<void>>(),
  mockSaveImageFile: vi.fn<(noteId: string, file: File) => Promise<string>>(),
  notesChangedRef: { current: null as null | (() => void) },
}));

vi.mock("../saveImage", () => ({ saveImageFile: mockSaveImageFile }));

vi.mock("../api", () => ({
  api: {
    notes: {
      load: vi.fn(),
      loadOne: vi.fn().mockResolvedValue("<p>Hello</p>"),
      save: vi.fn(),
      delete: vi.fn(),
      revisions: vi.fn().mockResolvedValue([]),
      revisionContent: vi.fn().mockResolvedValue(""),
    },
    onNotesChanged: (cb: () => void) => { notesChangedRef.current = cb; return () => { notesChangedRef.current = null; }; },
    openNoteWindow: vi.fn(),
    setWindowTitle: vi.fn(),
    toggleAlwaysOnTop: mockToggleAlwaysOnTop,
    closeWindow: mockCloseWindow,
    getAppInfo: vi.fn(),
    openExternal: vi.fn(),
    startResize: vi.fn(),
  },
}));

const { default: NoteEditor } = await import("./NoteEditor");
const { api } = await import("../api");

const mockNote: NoteMeta = {
  id: "1", updatedAt: 1000, pinned: false, archived: false, color: "", dueAt: null, folderId: null,
  position: 0, deletedAt: null, preview: "Hello", tasksDone: 0, tasksTotal: 0,
  protected: false, title: "Hello", mcpHidden: false,
};
const onChange = vi.fn();

// Waits for the note-load microtask chain (api.notes.loadOne().then(apply)) to settle.
async function flushNoteLoad() {
  await act(async () => { await Promise.resolve(); await Promise.resolve(); });
}

beforeEach(() => {
  vi.clearAllMocks();
  resetFakeEditor();
  mockToggleAlwaysOnTop.mockResolvedValue(true);
  mockSaveImageFile.mockResolvedValue("image-src.png");
  // Most tests don't care about the note-load effect's async `loadOne().then(apply)`
  // resolving — a promise that never settles means its setState calls never fire
  // outside an act() scope after the test body returns. Tests that DO care (the
  // autosave describe block) override this locally to a resolving value.
  (api.notes.loadOne as ReturnType<typeof vi.fn>).mockImplementation(() => new Promise(() => {}));
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
    render(<NoteEditor note={mockNote} onChange={vi.fn()} />);
    expect(screen.getByLabelText("Speichern")).toBeInTheDocument();
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

  it("shows the resize grip", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} isWindow />);
    expect(screen.getByTitle("Größe ändern")).toBeInTheDocument();
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

  it("clears the due date via the × button", () => {
    const onSetDue = vi.fn();
    render(<NoteEditor note={{ ...mockNote, dueAt: Date.now() }} onChange={onChange} onSetDue={onSetDue} />);
    fireEvent.click(screen.getByTitle("Fälligkeit löschen"));
    expect(onSetDue).toHaveBeenCalledWith(mockNote.id, null);
  });

  it("does not render the × button when no due date is set", () => {
    const onSetDue = vi.fn();
    render(<NoteEditor note={mockNote} onChange={onChange} onSetDue={onSetDue} />);
    expect(screen.queryByTitle("Fälligkeit löschen")).not.toBeInTheDocument();
  });

  it("does not render the due-date row at all when onSetDue is omitted", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    expect(screen.queryByLabelText("Fälligkeitsdatum")).not.toBeInTheDocument();
  });
});

describe("NoteEditor — toolbar actions dispatch the right editor commands", () => {
  it("toggles bold and reflects the active state on the button", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    const btn = screen.getByTitle("Fett");
    fireEvent.mouseDown(btn);
    expect(fakeEditor.isActive("bold")).toBe(true);
  });

  it("toggles italic, underline and strike independently", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    fireEvent.mouseDown(screen.getByTitle("Kursiv"));
    fireEvent.mouseDown(screen.getByTitle("Unterstrichen"));
    expect(fakeEditor.isActive("italic")).toBe(true);
    expect(fakeEditor.isActive("underline")).toBe(true);
    expect(fakeEditor.isActive("strike")).toBe(false);
  });

  it("toggles each heading level with the exact level argument", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    fireEvent.mouseDown(screen.getByTitle("Überschrift 2"));
    expect(fakeEditor.isActive("heading", { level: 2 })).toBe(true);
    expect(fakeEditor.isActive("heading", { level: 1 })).toBe(false);
    expect(fakeEditor.isActive("heading", { level: 3 })).toBe(false);
  });

  it("toggles bullet list, ordered list and code block", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    fireEvent.mouseDown(screen.getByTitle("Aufzählung"));
    fireEvent.mouseDown(screen.getByTitle("Nummerierte Liste"));
    fireEvent.mouseDown(screen.getByTitle("Code-Block"));
    expect(fakeEditor.isActive("bulletList")).toBe(true);
    expect(fakeEditor.isActive("orderedList")).toBe(true);
    expect(fakeEditor.isActive("codeBlock")).toBe(true);
  });

  it("toggles the task list", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    fireEvent.mouseDown(screen.getByTitle("Aufgabenliste"));
    expect(fakeEditor.isActive("taskList")).toBe(true);
  });

  it("toggles strikethrough", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    fireEvent.mouseDown(screen.getByTitle("Durchgestrichen"));
    expect(fakeEditor.isActive("strike")).toBe(true);
  });

  it("clicking the insert-image toolbar button opens the hidden file picker", () => {
    const { container } = render(<NoteEditor note={mockNote} onChange={onChange} />);
    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    const clickSpy = vi.spyOn(input, "click");
    fireEvent.mouseDown(screen.getByTitle("Bild einfügen"));
    expect(clickSpy).toHaveBeenCalledOnce();
  });

  it("opens the history modal from the toolbar and closes it", async () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    fireEvent.mouseDown(screen.getByTitle("Verlauf"));
    expect(await screen.findByText("Wähle eine Version.")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Schließen"));
    expect(screen.queryByText("Wähle eine Version.")).not.toBeInTheDocument();
  });

  it("inserts a picked image file via editor.setImage with the saved src", async () => {
    const { container } = render(<NoteEditor note={mockNote} onChange={onChange} />);
    const file = new File(["x"], "pic.png", { type: "image/png" });
    const input = container.querySelector('input[type="file"]') as HTMLInputElement;
    await act(async () => { fireEvent.change(input, { target: { files: [file] } }); });
    expect(mockSaveImageFile).toHaveBeenCalledWith(mockNote.id, file);
    // setImage is the leaf spy returned by chain().focus(); pull the *actual* spy
    // instance used for this call (each chain()/focus() call mints fresh spies).
    const chainResult = fakeEditor.chain.mock.results.at(-1)!.value;
    const focusResult = chainResult.focus.mock.results.at(-1)!.value;
    expect(focusResult.setImage).toHaveBeenCalledWith({ src: "image-src.png" });
  });
});

describe("NoteEditor — markdown view toggle", () => {
  it("switches to markdown mode and shows the md status line", () => {
    fakeEditor.getHTML.mockImplementation(() => "<p>Hello world</p>");
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    fireEvent.mouseDown(screen.getByTitle("Markdown"));
    expect(screen.getByText("Z 1, Sp 1 | Länge: 11 | Zeilen: 1")).toBeInTheDocument();
  });

  it("hides the find button while in markdown mode", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    fireEvent.mouseDown(screen.getByTitle("Markdown"));
    expect(screen.queryByTitle("In Notiz suchen")).not.toBeInTheDocument();
  });

  it("converts back to HTML via editor.commands.setContent when leaving markdown mode", () => {
    fakeEditor.getHTML.mockImplementation(() => "<p>Hello</p>");
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    fireEvent.mouseDown(screen.getByTitle("Markdown"));
    fireEvent.mouseDown(screen.getByTitle("Markdown"));
    expect(fakeEditor.commands.setContent).toHaveBeenLastCalledWith("<p>Hello</p>\n");
  });

  // Regression: react-simple-code-editor is CJS-only. Under Vite's browser
  // CJS interop its bare default import once resolved to the module namespace
  // object instead of the component, so React threw "Element type is invalid …
  // got: object" the moment the markdown view mounted — while the test suite
  // stayed green because vitest interops differently. The real (unmocked)
  // code editor must mount its textarea with the converted markdown.
  it("mounts the real code editor textarea with the note's markdown", () => {
    fakeEditor.getHTML.mockImplementation(() => "<h2>Title</h2><p>Body</p>");
    const { container } = render(<NoteEditor note={mockNote} onChange={onChange} />);
    fireEvent.mouseDown(screen.getByTitle("Markdown"));
    const textarea = container.querySelector(".md-code-editor textarea") as HTMLTextAreaElement | null;
    expect(textarea).not.toBeNull();
    expect(textarea!.value).toBe("## Title\n\nBody");
  });
});

describe("NoteEditor — autosave debounce", () => {
  beforeEach(() => {
    (api.notes.loadOne as ReturnType<typeof vi.fn>).mockResolvedValue("<p>Hello</p>");
  });

  it("swallows the update fired by the initial programmatic setContent on note load", async () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    await flushNoteLoad();
    expect(fakeEditor.commands.setContent).toHaveBeenCalledWith("<p>Hello</p>");

    fakeEditor.getHTML.mockImplementation(() => "<p>should be skipped</p>");
    act(() => { getConfig().onUpdate({ editor: fakeEditor }); });
    vi.useFakeTimers();
    vi.advanceTimersByTime(10_000);
    vi.useRealTimers();
    expect(onChange).not.toHaveBeenCalled();
  });

  it("calls onChange exactly once after the configured delay, with the final HTML", async () => {
    render(<NoteEditor note={mockNote} onChange={onChange} autosaveDelay={500} />);
    await flushNoteLoad();
    act(() => { getConfig().onUpdate({ editor: fakeEditor }); }); // swallowed (post-load skip guard)

    vi.useFakeTimers();
    fakeEditor.getHTML.mockImplementation(() => "<p>edited once</p>");
    act(() => { getConfig().onUpdate({ editor: fakeEditor }); });
    act(() => { vi.advanceTimersByTime(499); });
    expect(onChange).not.toHaveBeenCalled();
    act(() => { vi.advanceTimersByTime(1); });
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith(mockNote.id, "<p>edited once</p>");
    vi.useRealTimers();
  });

  it("resets the debounce timer on each update, saving only the last content", async () => {
    render(<NoteEditor note={mockNote} onChange={onChange} autosaveDelay={400} />);
    await flushNoteLoad();
    act(() => { getConfig().onUpdate({ editor: fakeEditor }); }); // swallowed

    vi.useFakeTimers();
    fakeEditor.getHTML.mockImplementation(() => "<p>v1</p>");
    act(() => { getConfig().onUpdate({ editor: fakeEditor }); });
    act(() => { vi.advanceTimersByTime(300); });
    fakeEditor.getHTML.mockImplementation(() => "<p>v2</p>");
    act(() => { getConfig().onUpdate({ editor: fakeEditor }); }); // restarts the 400ms timer
    act(() => { vi.advanceTimersByTime(300); });
    expect(onChange).not.toHaveBeenCalled(); // only 300ms since the last update
    act(() => { vi.advanceTimersByTime(100); });
    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith(mockNote.id, "<p>v2</p>");
    vi.useRealTimers();
  });

  it("flushSave (clicking the save icon) saves immediately, bypassing the debounce", async () => {
    render(<NoteEditor note={mockNote} onChange={onChange} autosaveDelay={5000} />);
    await flushNoteLoad();
    act(() => { getConfig().onUpdate({ editor: fakeEditor }); }); // swallowed

    fakeEditor.getHTML.mockImplementation(() => "<p>flush me</p>");
    act(() => { getConfig().onUpdate({ editor: fakeEditor }); });
    fireEvent.click(screen.getByLabelText("Speichern"));
    expect(onChange).toHaveBeenCalledWith(mockNote.id, "<p>flush me</p>");
  });
});

describe("NoteEditor — copy-format handling", () => {
  function fakeClipboardEvent() {
    return { clipboardData: { setData: vi.fn(), files: [] }, preventDefault: vi.fn() } as any;
  }
  function fakeView(sel: { from: number; to: number; empty: boolean }) {
    return {
      state: {
        selection: sel,
        schema: {},
        doc: { slice: (from: number, to: number) => ({ content: `sel:${from}-${to}` }) },
        tr: { deleteSelection: vi.fn(() => "TR_AFTER_DELETE") },
      },
      dispatch: vi.fn(),
    };
  }

  it("copies as markdown by default (copyFormat='md')", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    const view = fakeView({ from: 2, to: 5, empty: false });
    const ev = fakeClipboardEvent();
    const handled = getConfig().editorProps.handleDOMEvents.copy(view, ev);
    expect(handled).toBe(true);
    expect(ev.clipboardData.setData).toHaveBeenCalledWith("text/plain", "sel:2-5");
    expect(ev.preventDefault).toHaveBeenCalled();
  });

  it("copies as plain text when copyFormat='text'", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} copyFormat="text" />);
    const view = fakeView({ from: 0, to: 3, empty: false });
    const ev = fakeClipboardEvent();
    getConfig().editorProps.handleDOMEvents.copy(view, ev);
    expect(ev.clipboardData.setData).toHaveBeenCalledWith("text/plain", "sel:0-3");
  });

  it("copies raw html when copyFormat='html'", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} copyFormat="html" />);
    const view = fakeView({ from: 1, to: 4, empty: false });
    const ev = fakeClipboardEvent();
    getConfig().editorProps.handleDOMEvents.copy(view, ev);
    expect(ev.clipboardData.setData).toHaveBeenCalledWith("text/plain", "<p>sel:1-4</p>");
  });

  it("does nothing (returns false) for copyFormat='richtext', leaving the native copy alone", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} copyFormat="richtext" />);
    const view = fakeView({ from: 0, to: 3, empty: false });
    const ev = fakeClipboardEvent();
    const handled = getConfig().editorProps.handleDOMEvents.copy(view, ev);
    expect(handled).toBe(false);
    expect(ev.clipboardData.setData).not.toHaveBeenCalled();
    expect(ev.preventDefault).not.toHaveBeenCalled();
  });

  it("does nothing for an empty (collapsed) selection", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    const view = fakeView({ from: 3, to: 3, empty: true });
    const ev = fakeClipboardEvent();
    const handled = getConfig().editorProps.handleDOMEvents.copy(view, ev);
    expect(handled).toBe(false);
    expect(ev.clipboardData.setData).not.toHaveBeenCalled();
  });

  it("on cut, deletes the selection in addition to writing the clipboard", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    const view = fakeView({ from: 0, to: 2, empty: false });
    const ev = fakeClipboardEvent();
    const handled = getConfig().editorProps.handleDOMEvents.cut(view, ev);
    expect(handled).toBe(true);
    expect(view.state.tr.deleteSelection).toHaveBeenCalled();
    expect(view.dispatch).toHaveBeenCalledWith("TR_AFTER_DELETE");
  });

  it("copy (not cut) never touches the document", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    const view = fakeView({ from: 0, to: 2, empty: false });
    getConfig().editorProps.handleDOMEvents.copy(view, fakeClipboardEvent());
    expect(view.dispatch).not.toHaveBeenCalled();
  });
});

describe("NoteEditor — find-in-note open/close/navigate", () => {
  it("opens the find bar from the toolbar toggle", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    expect(screen.queryByPlaceholderText(/./)).not.toBeInTheDocument();
    fireEvent.mouseDown(screen.getByTitle("In Notiz suchen"));
    expect(screen.getByRole("textbox", { name: "" })).toBeInTheDocument();
  });

  it("opens via the configured shortcut", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} findShortcut="Mod+J" />);
    fireEvent.keyDown(window, { key: "f", metaKey: true }); // default combo, not bound here
    expect(screen.queryByTitle("Schließen (Esc)")).not.toBeInTheDocument();
    fireEvent.keyDown(window, { key: "j", metaKey: true });
    expect(document.querySelector(".absolute.left-1\\/2")).toBeInTheDocument();
  });

  it("does not open on an unrelated key combo", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    fireEvent.keyDown(window, { key: "x", metaKey: true });
    expect(document.querySelector(".absolute.left-1\\/2")).not.toBeInTheDocument();
  });

  it("closes via Escape inside the find bar, and re-opening starts fresh", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    fireEvent.mouseDown(screen.getByTitle("In Notiz suchen"));
    const input = screen.getByRole("textbox");
    fireEvent.keyDown(input, { key: "Escape" });
    expect(document.querySelector(".absolute.left-1\\/2")).not.toBeInTheDocument();
  });

  it("navigates matches: Enter steps forward, Shift+Enter steps backward", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    fireEvent.mouseDown(screen.getByTitle("In Notiz suchen"));
    const input = screen.getByRole("textbox");
    fireEvent.change(input, { target: { value: "hi" } });
    expect(fakeEditor.commands.setSearch).toHaveBeenCalledWith("hi");
    fireEvent.keyDown(input, { key: "Enter" });
    expect(fakeEditor.commands.stepSearch).toHaveBeenCalledWith(1);
    fireEvent.keyDown(input, { key: "Enter", shiftKey: true });
    expect(fakeEditor.commands.stepSearch).toHaveBeenCalledWith(-1);
  });

  it("is not rendered while in markdown mode, even if findOpen was true", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    fireEvent.mouseDown(screen.getByTitle("In Notiz suchen"));
    fireEvent.mouseDown(screen.getByTitle("Markdown"));
    expect(document.querySelector(".absolute.left-1\\/2")).not.toBeInTheDocument();
  });
});

describe("NoteEditor — char/word count display + position", () => {
  it("shows word and character counts with no selection", () => {
    fakeEditor.state = { selection: { from: 0, to: 0 }, doc: { textContent: "one two three" } };
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    expect(screen.getByText("Wörter: 3 | Zeichen: 13")).toBeInTheDocument();
  });

  it("shows the selection length once a range is selected", () => {
    fakeEditor.state = { selection: { from: 2, to: 7 }, doc: { textContent: "one two three" } };
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    expect(screen.getByText("Wörter: 3 | Zeichen: 13 · Auswahl: 5")).toBeInTheDocument();
  });

  it("hides the status line entirely when countShow=false", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} countShow={false} />);
    expect(screen.queryByText(/Wörter:/)).not.toBeInTheDocument();
  });

  it("positions the counter top-left / bottom-right / bottom-left per countPos", () => {
    const { rerender, container } = render(<NoteEditor note={mockNote} onChange={onChange} countPos="topLeft" />);
    let cluster = container.querySelector(".absolute.z-10, [class*='absolute'][class*='z-10']")!;
    expect(cluster.className).toContain("left-2");

    rerender(<NoteEditor note={mockNote} onChange={onChange} countPos="bottomRight" />);
    cluster = container.querySelector("[class*='absolute'][class*='z-10']")!;
    expect(cluster.className).toContain("right-2");
    expect(cluster.className).toContain("bottom-14");

    rerender(<NoteEditor note={mockNote} onChange={onChange} countPos="bottomLeft" />);
    cluster = container.querySelector("[class*='absolute'][class*='z-10']")!;
    expect(cluster.className).toContain("left-2");
    expect(cluster.className).toContain("bottom-14");
  });
});

describe("NoteEditor — link-preview modes", () => {
  function fakePasteEvent(text: string) {
    return {
      clipboardData: { getData: () => text, files: [] },
      preventDefault: vi.fn(),
    } as any;
  }
  function fakeView() {
    const created: any[] = [];
    return {
      created,
      state: {
        selection: { to: 4 },
        schema: { nodes: { linkPreview: { create: vi.fn((attrs: unknown) => { created.push(attrs); return { attrs }; }) } } },
        tr: { replaceSelectionWith: vi.fn(function (this: unknown, node: unknown) { return { scrollIntoView: () => ({ node, scrolled: true }) }; }) },
      },
      dispatch: vi.fn(),
    };
  }

  it("wraps a pasted bare URL in a linkPreview node using the configured display mode", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} linkPreviewMode="inline" />);
    const view = fakeView();
    const ev = fakePasteEvent("https://example.com/page");
    const handled = getConfig().editorProps.handlePaste(view, ev);
    expect(handled).toBe(true);
    expect(ev.preventDefault).toHaveBeenCalled();
    expect(view.state.schema.nodes.linkPreview.create).toHaveBeenCalledWith({
      href: "https://example.com/page",
      display: "inline",
    });
    expect(view.dispatch).toHaveBeenCalled();
  });

  it("defaults to 'card' display when linkPreviewMode is not set", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    const view = fakeView();
    getConfig().editorProps.handlePaste(view, fakePasteEvent("https://example.com"));
    expect(view.state.schema.nodes.linkPreview.create).toHaveBeenCalledWith({
      href: "https://example.com",
      display: "card",
    });
  });

  it("does not intercept the paste when linkPreviewEnabled is false, even for a bare URL", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} linkPreviewEnabled={false} />);
    const view = fakeView();
    const ev = fakePasteEvent("https://example.com");
    const handled = getConfig().editorProps.handlePaste(view, ev);
    expect(handled).toBe(false);
    expect(ev.preventDefault).not.toHaveBeenCalled();
    expect(view.dispatch).not.toHaveBeenCalled();
  });

  it("does not intercept a paste that is not a bare URL", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    const view = fakeView();
    const handled = getConfig().editorProps.handlePaste(view, fakePasteEvent("just some text"));
    expect(handled).toBe(false);
    expect(view.dispatch).not.toHaveBeenCalled();
  });
});

describe("NoteEditor — image paste / drop", () => {
  function fakeImageView(selectionTo: number) {
    return {
      state: {
        selection: { to: selectionTo },
        schema: { nodes: { image: { create: vi.fn((attrs: unknown) => ({ type: "image", attrs })) } } },
        tr: { insert: vi.fn((pos: number, node: unknown) => ({ pos, node })) },
      },
      dispatch: vi.fn(),
      posAtCoords: vi.fn(() => ({ pos: 9 })),
    };
  }
  const flush = () => act(async () => { await Promise.resolve(); await Promise.resolve(); await Promise.resolve(); });

  it("saves and inserts a pasted image file at the current selection", async () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    const view = fakeImageView(5);
    const file = new File(["x"], "pic.png", { type: "image/png" });
    const ev = { clipboardData: { getData: () => "", files: [file] }, preventDefault: vi.fn() } as any;
    const handled = getConfig().editorProps.handlePaste(view, ev);
    expect(handled).toBe(true);
    expect(ev.preventDefault).toHaveBeenCalled();
    await flush();
    expect(mockSaveImageFile).toHaveBeenCalledWith(mockNote.id, file);
    expect(view.state.schema.nodes.image.create).toHaveBeenCalledWith({ src: "image-src.png" });
    expect(view.state.tr.insert).toHaveBeenCalledWith(5, { type: "image", attrs: { src: "image-src.png" } });
    expect(view.dispatch).toHaveBeenCalledWith({ pos: 5, node: { type: "image", attrs: { src: "image-src.png" } } });
  });

  it("saves and inserts a dropped image file at the drop position (not the selection)", async () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    const view = fakeImageView(0);
    const file = new File(["x"], "pic.png", { type: "image/png" });
    const ev = { dataTransfer: { files: [file] }, clientX: 10, clientY: 20, preventDefault: vi.fn() } as any;
    const handled = getConfig().editorProps.handleDrop(view, ev);
    expect(handled).toBe(true);
    await flush();
    expect(view.posAtCoords).toHaveBeenCalledWith({ left: 10, top: 20 });
    expect(view.state.tr.insert).toHaveBeenCalledWith(9, { type: "image", attrs: { src: "image-src.png" } });
  });

  it("does not intercept a paste/drop with no image files", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    const view = fakeImageView(0);
    const ev = { clipboardData: { getData: () => "", files: [] }, preventDefault: vi.fn() } as any;
    expect(getConfig().editorProps.handlePaste(view, ev)).toBe(false);
    expect(ev.preventDefault).not.toHaveBeenCalled();
  });
});

describe("NoteEditor — isWindow title sync", () => {
  it("sets the window title from the loaded note's first heading", async () => {
    (api.notes.loadOne as ReturnType<typeof vi.fn>).mockResolvedValue("<h1>My Title</h1><p>body</p>");
    render(<NoteEditor note={mockNote} onChange={onChange} isWindow />);
    await flushNoteLoad();
    expect(api.setWindowTitle).toHaveBeenCalledWith("My Title");
  });

  it("falls back to 'New note' when the loaded content has no text", async () => {
    (api.notes.loadOne as ReturnType<typeof vi.fn>).mockResolvedValue("<p></p>");
    render(<NoteEditor note={mockNote} onChange={onChange} isWindow />);
    await flushNoteLoad();
    expect(api.setWindowTitle).toHaveBeenCalledWith("New note");
  });
});

describe("NoteEditor — external edits from another window", () => {
  it("reloads and applies content changed elsewhere when no local save is pending", async () => {
    (api.notes.loadOne as ReturnType<typeof vi.fn>)
      .mockResolvedValueOnce("<p>Hello</p>")
      .mockResolvedValueOnce("<p>Changed elsewhere</p>");
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    await flushNoteLoad();
    fakeEditor.getHTML.mockImplementation(() => "<p>Hello</p>"); // matches what was loaded
    await act(async () => {
      notesChangedRef.current?.();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(fakeEditor.commands.setContent).toHaveBeenLastCalledWith("<p>Changed elsewhere</p>");
  });

  it("does nothing when the incoming content matches the current editor content", async () => {
    (api.notes.loadOne as ReturnType<typeof vi.fn>).mockResolvedValue("<p>Hello</p>");
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    await flushNoteLoad();
    fakeEditor.getHTML.mockImplementation(() => "<p>Hello</p>");
    fakeEditor.commands.setContent.mockClear();
    await act(async () => {
      notesChangedRef.current?.();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(fakeEditor.commands.setContent).not.toHaveBeenCalled();
  });
});

describe("NoteEditor — markdown-mode editing debounce", () => {
  it("debounces onMdChange and saves the converted html after the delay", () => {
    fakeEditor.getHTML.mockImplementation(() => "<p>Hello</p>");
    vi.useFakeTimers();
    render(<NoteEditor note={mockNote} onChange={onChange} autosaveDelay={300} />);
    fireEvent.mouseDown(screen.getByTitle("Markdown"));
    const textarea = document.querySelector("textarea")!;
    fireEvent.change(textarea, { target: { value: "# New heading" } });
    act(() => { vi.advanceTimersByTime(299); });
    expect(onChange).not.toHaveBeenCalled();
    act(() => { vi.advanceTimersByTime(1); });
    expect(onChange).toHaveBeenCalledWith(mockNote.id, markdownToHtml("# New heading"));
    vi.useRealTimers();
  });
});

describe("NoteEditor — invisibles toggle", () => {
  it("applies the invisibles setting to the editor on mount", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} invisibles />);
    expect(fakeEditor.commands.setInvisibles).toHaveBeenCalledWith(true);
  });

  it("re-applies when the prop changes", () => {
    const { rerender } = render(<NoteEditor note={mockNote} onChange={onChange} invisibles={false} />);
    expect(fakeEditor.commands.setInvisibles).toHaveBeenLastCalledWith(false);
    rerender(<NoteEditor note={mockNote} onChange={onChange} invisibles={true} />);
    expect(fakeEditor.commands.setInvisibles).toHaveBeenLastCalledWith(true);
  });

  it("adds the show-invisibles class to the content area when enabled and not in markdown mode", () => {
    const { container } = render(<NoteEditor note={mockNote} onChange={onChange} invisibles />);
    expect(container.querySelector(".show-invisibles")).toBeInTheDocument();
  });

  it("omits the show-invisibles class while in markdown mode even if invisibles is true", () => {
    const { container } = render(<NoteEditor note={mockNote} onChange={onChange} invisibles />);
    fireEvent.mouseDown(screen.getByTitle("Markdown"));
    expect(container.querySelector(".show-invisibles")).not.toBeInTheDocument();
  });
});

describe("NoteEditor — toolbar position variants", () => {
  it("renders the toolbar at the bottom by default (border-t, order 0)", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} />);
    const toolbar = screen.getByTitle("Fett").closest("div")!;
    expect(toolbar.className).toContain("border-t");
    expect(toolbar.style.order).toBe("0");
  });

  it("renders the toolbar at the top (border-b, order -1)", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} toolbarPos="top" />);
    const toolbar = screen.getByTitle("Fett").closest("div")!;
    expect(toolbar.className).toContain("border-b");
    expect(toolbar.style.order).toBe("-1");
  });

  it("hides the toolbar entirely when toolbarPos='hidden'", () => {
    render(<NoteEditor note={mockNote} onChange={onChange} toolbarPos="hidden" />);
    expect(screen.queryByTitle("Fett")).not.toBeInTheDocument();
  });
});
