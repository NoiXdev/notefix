import { renderHook, act, waitFor } from "@testing-library/react";
import { describe, it, expect, beforeEach, vi } from "vitest";
import type { Note } from "../types";

const { mockLoad, mockSave, mockDeleteFn, mockSetPinned, mockSetArchived, mockSetColor, mockSetDue, mockSetFolder, mockReorder, mockRestore, mockPurge, mockTrashLoad, mockTrashEmpty, setOnChanged } = vi.hoisted(() => {
  let cb: (() => void) | null = null;
  return {
    mockLoad: vi.fn<() => Promise<Note[]>>(),
    mockSave: vi.fn<(note: Note) => Promise<void>>(),
    mockDeleteFn: vi.fn<(id: string) => Promise<void>>(),
    mockSetPinned: vi.fn<(id: string, pinned: boolean) => Promise<void>>(),
    mockSetArchived: vi.fn<(id: string, archived: boolean) => Promise<void>>(),
    mockSetColor: vi.fn<(id: string, color: string) => Promise<void>>(),
    mockSetDue: vi.fn<(id: string, dueAt: number | null) => Promise<void>>(),
    mockSetFolder: vi.fn<(id: string, folderId: string | null) => Promise<void>>(),
    mockReorder: vi.fn<(folderId: string | null, ids: string[]) => Promise<void>>(),
    mockRestore: vi.fn<(id: string) => Promise<void>>(),
    mockPurge: vi.fn<(id: string) => Promise<void>>(),
    mockTrashLoad: vi.fn<() => Promise<Note[]>>(),
    mockTrashEmpty: vi.fn<() => Promise<void>>(),
    setOnChanged: { get: () => cb, set: (f: (() => void) | null) => (cb = f) },
  };
});

vi.mock("../api", () => ({
  api: {
    notes: { load: mockLoad, save: mockSave, delete: mockDeleteFn, setPinned: mockSetPinned, setArchived: mockSetArchived, setColor: mockSetColor, setDue: mockSetDue, setFolder: mockSetFolder, reorder: mockReorder, restore: mockRestore, purge: mockPurge },
    trash: { load: mockTrashLoad, empty: mockTrashEmpty },
    onNotesChanged: (cb: () => void) => {
      setOnChanged.set(cb);
      return () => {};
    },
  },
}));

import { useNotes } from "./useNotes";

beforeEach(() => {
  setOnChanged.set(null);
  vi.clearAllMocks();
  mockLoad.mockResolvedValue([]);
  mockSave.mockResolvedValue(undefined);
  mockDeleteFn.mockResolvedValue(undefined);
  mockSetPinned.mockResolvedValue(undefined);
  mockSetArchived.mockResolvedValue(undefined);
  mockSetColor.mockResolvedValue(undefined);
  mockSetDue.mockResolvedValue(undefined);
  mockSetFolder.mockResolvedValue(undefined);
  mockReorder.mockResolvedValue(undefined);
  mockRestore.mockResolvedValue(undefined);
  mockPurge.mockResolvedValue(undefined);
  mockTrashLoad.mockResolvedValue([]);
  mockTrashEmpty.mockResolvedValue(undefined);
});

async function rendered(initial: Note[] = []) {
  mockLoad.mockResolvedValue([...initial]);
  const hook = renderHook(() => useNotes());
  await waitFor(() => expect(hook.result.current.loading).toBe(false));
  return hook;
}

describe("useNotes — initial state", () => {
  it("starts with loading true then resolves to false", async () => {
    const { result } = renderHook(() => useNotes());
    expect(result.current.loading).toBe(true);
    await waitFor(() => expect(result.current.loading).toBe(false));
  });

  it("starts empty when no notes exist", async () => {
    const { result } = await rendered();
    expect(result.current.notes).toEqual([]);
  });

  it("loads existing notes on mount", async () => {
    const stored: Note[] = [{ id: "abc", content: "<p>hello</p>", updatedAt: 1000, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0 }];
    const { result } = await rendered(stored);
    expect(result.current.notes).toHaveLength(1);
    expect(result.current.notes[0].id).toBe("abc");
  });
});

describe("useNotes — createNote", () => {
  it("adds a note and returns its id", async () => {
    const { result } = await rendered();
    let id!: string;
    await act(async () => { id = await result.current.createNote(); });
    expect(result.current.notes).toHaveLength(1);
    expect(result.current.notes[0].id).toBe(id);
  });

  it("creates a note with empty content", async () => {
    const { result } = await rendered();
    await act(async () => { await result.current.createNote(); });
    expect(result.current.notes[0].content).toBe("");
  });

  it("calls notes.save with the new note", async () => {
    const { result } = await rendered();
    await act(async () => { await result.current.createNote(); });
    expect(mockSave).toHaveBeenCalledOnce();
    expect(mockSave.mock.calls[0][0]).toMatchObject({ content: "" });
  });
});

describe("useNotes — updateNote", () => {
  it("updates the content of the matching note", async () => {
    const { result } = await rendered();
    let id!: string;
    await act(async () => { id = await result.current.createNote(); });
    await act(async () => { await result.current.updateNote(id, "<p>updated</p>"); });
    expect(result.current.notes[0].content).toBe("<p>updated</p>");
  });

  it("calls notes.save with the updated note", async () => {
    const { result } = await rendered();
    let id!: string;
    await act(async () => { id = await result.current.createNote(); });
    mockSave.mockClear();
    await act(async () => { await result.current.updateNote(id, "<p>saved</p>"); });
    expect(mockSave).toHaveBeenCalledOnce();
    expect(mockSave.mock.calls[0][0]).toMatchObject({ id, content: "<p>saved</p>" });
  });
});

describe("useNotes — deleteNote", () => {
  it("removes the note with the given id", async () => {
    const { result } = await rendered();
    let id!: string;
    await act(async () => { id = await result.current.createNote(); });
    await act(async () => { await result.current.deleteNote(id); });
    expect(result.current.notes).toHaveLength(0);
  });

  it("calls notes.delete with the correct id", async () => {
    const { result } = await rendered();
    let id!: string;
    await act(async () => { id = await result.current.createNote(); });
    await act(async () => { await result.current.deleteNote(id); });
    expect(mockDeleteFn).toHaveBeenCalledWith(id);
  });
});

describe("useNotes — cross-window sync", () => {
  it("reloads notes when onNotesChanged fires", async () => {
    const { result } = await rendered();
    const external: Note[] = [{ id: "ext", content: "<p>from other window</p>", updatedAt: 9999, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0 }];
    mockLoad.mockResolvedValue(external);
    await act(async () => { setOnChanged.get()?.(); });
    await waitFor(() => expect(result.current.notes[0]?.id).toBe("ext"));
  });
});

describe("useNotes — pinning", () => {
  it("setPinned floats the note to the top", async () => {
    const stored: Note[] = [
      { id: "a", content: "<p>a</p>", updatedAt: 2000, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0 },
      { id: "b", content: "<p>b</p>", updatedAt: 1000, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 1 },
    ];
    const { result } = await rendered(stored);
    await act(async () => { await result.current.setPinned("b", true); });
    expect(result.current.notes[0].id).toBe("b");
    expect(result.current.notes[0].pinned).toBe(true);
    expect(mockSetPinned).toHaveBeenCalledWith("b", true);
  });

  it("updateNote preserves the pinned flag", async () => {
    const stored: Note[] = [{ id: "a", content: "<p>a</p>", updatedAt: 1000, pinned: true, archived: false, color: '', dueAt: null, folderId: null, position: 0 }];
    const { result } = await rendered(stored);
    await act(async () => { await result.current.updateNote("a", "<p>edited</p>"); });
    expect(result.current.notes[0].pinned).toBe(true);
    expect(result.current.notes[0].content).toBe("<p>edited</p>");
  });
});

describe("useNotes — archive & color", () => {
  it("setArchived flips the flag and calls the bridge", async () => {
    const { result } = await rendered([{ id: "a", content: "<p>a</p>", updatedAt: 1, pinned: false, archived: false, color: "", dueAt: null, folderId: null, position: 0 }]);
    await act(async () => { await result.current.setArchived("a", true); });
    expect(result.current.notes[0].archived).toBe(true);
    expect(mockSetArchived).toHaveBeenCalledWith("a", true);
  });

  it("setColor sets the color and calls the bridge", async () => {
    const { result } = await rendered([{ id: "a", content: "<p>a</p>", updatedAt: 1, pinned: false, archived: false, color: "", dueAt: null, folderId: null, position: 0 }]);
    await act(async () => { await result.current.setColor("a", "#ef4444"); });
    expect(result.current.notes[0].color).toBe("#ef4444");
    expect(mockSetColor).toHaveBeenCalledWith("a", "#ef4444");
  });
});

describe("useNotes — due date", () => {
  it("setDue sets and clears the due date", async () => {
    const { result } = await rendered([{ id: "a", content: "<p>a</p>", updatedAt: 1, pinned: false, archived: false, color: "", dueAt: null, folderId: null, position: 0 }]);
    await act(async () => { await result.current.setDue("a", 5000); });
    expect(result.current.notes[0].dueAt).toBe(5000);
    expect(mockSetDue).toHaveBeenCalledWith("a", 5000);
    await act(async () => { await result.current.setDue("a", null); });
    expect(result.current.notes[0].dueAt).toBe(null);
  });
});

describe("useNotes — folder", () => {
  it("setFolder moves a note and clears it", async () => {
    const { result } = await rendered([{ id: "a", content: "<p>a</p>", updatedAt: 1, pinned: false, archived: false, color: "", dueAt: null, folderId: null, position: 0 }]);
    await act(async () => { await result.current.setFolder("a", "f1"); });
    expect(result.current.notes[0].folderId).toBe("f1");
    await act(async () => { await result.current.setFolder("a", null); });
    expect(result.current.notes[0].folderId).toBe(null);
  });
});

describe("useNotes — reorder", () => {
  it("reorderNotes assigns folder + position and calls the bridge", async () => {
    const { result } = await rendered([
      { id: "a", content: "<p>a</p>", updatedAt: 2, pinned: false, archived: false, color: "", dueAt: null, folderId: null, position: 0 },
      { id: "b", content: "<p>b</p>", updatedAt: 1, pinned: false, archived: false, color: "", dueAt: null, folderId: null, position: 1 },
    ]);
    await act(async () => { await result.current.reorderNotes("f1", ["b", "a"]); });
    expect(result.current.notes.map(x => x.id)).toEqual(["b", "a"]);
    expect(result.current.notes[0].folderId).toBe("f1");
    expect(mockReorder).toHaveBeenCalledWith("f1", ["b", "a"]);
  });
});
