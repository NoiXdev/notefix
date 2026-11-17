import { render, act } from "@testing-library/react";
import { StrictMode } from "react";
import { describe, it, expect, vi } from "vitest";
import type { NoteMeta } from "../types";

// Unlike NoteEditor.test.tsx this drives the REAL Tiptap editor: the bug it
// guards against lives in how Tiptap emits update events, which a fake
// editor cannot reproduce.
vi.mock("../api", () => ({
  api: {
    notes: { loadOne: vi.fn(async () => "<p>hello</p><p>world</p>") },
    onNotesChanged: () => () => {},
    onNotesSynced: () => () => {},
    setWindowTitle: vi.fn(),
    isAlwaysOnTop: vi.fn(async () => false),
    toggleAlwaysOnTop: vi.fn(),
    closeWindow: vi.fn(),
    startResize: vi.fn(),
    openNoteWindow: vi.fn(),
  },
}));

// jsdom has no layout; ProseMirror measures the selection when it scrolls.
const noRects = () => [] as unknown as DOMRectList;
Range.prototype.getClientRects = noRects;
Range.prototype.getBoundingClientRect = () => new DOMRect();
(Text.prototype as unknown as { getClientRects: () => DOMRectList }).getClientRects = noRects;

import NoteEditor from "./NoteEditor";

const note: NoteMeta = {
  id: "n1", updatedAt: 1, dirty: false, pinned: false, archived: false, color: "", dueAt: null,
  folderId: null, position: 0, deletedAt: null, preview: "", tasksDone: 0, tasksTotal: 0,
  protected: false, title: "", mcpHidden: false,
};

describe("NoteEditor — fresh mount with the real editor", () => {
  // A detached note window always mounts a fresh editor. `setEditable` used
  // to emit an update while it still held its empty placeholder, scheduling
  // an autosave of `<p></p>` that landed over the note after it had loaded.
  it.each([false, true])("never saves the empty placeholder over the note (isWindow=%s)", async (isWindow) => {
    const onChange = vi.fn();
    const { container } = render(
      <StrictMode><NoteEditor note={note} onChange={onChange} isWindow={isWindow} /></StrictMode>,
    );
    await act(async () => { await new Promise(r => setTimeout(r, 700)); });
    expect(container.querySelector(".ProseMirror")?.innerHTML).toBe("<p>hello</p><p>world</p>");
    expect(onChange).not.toHaveBeenCalled();
  });
});
