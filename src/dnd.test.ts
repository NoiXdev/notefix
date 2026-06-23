import { describe, it, expect } from "vitest";
import { computeDrop } from "./dnd";
import type { Note, Folder } from "./types";

const n = (id: string, folderId: string | null, position: number): Note =>
  ({ id, content: `<p>${id}</p>`, updatedAt: 1, pinned: false, archived: false, color: "", dueAt: null, folderId, position });
const f = (id: string, parentId: string | null, position: number): Folder => ({ id, name: id, parentId, position, icon: '', color: '' });

describe("computeDrop — notes", () => {
  it("drops a note into a folder (appended)", () => {
    const notes = [n("a", null, 0), n("b", "f1", 0)];
    const r = computeDrop({ draggedKind: "note", draggedId: "a", targetKind: "folder", targetId: "f1", mode: "into", notes, folders: [] });
    expect(r).toEqual({ kind: "note", parentId: "f1", orderedIds: ["b", "a"] });
  });
  it("reorders before another note", () => {
    const notes = [n("a", null, 0), n("b", null, 1), n("c", null, 2)];
    const r = computeDrop({ draggedKind: "note", draggedId: "c", targetKind: "note", targetId: "a", mode: "before", notes, folders: [] });
    expect(r).toEqual({ kind: "note", parentId: null, orderedIds: ["c", "a", "b"] });
  });
});

describe("computeDrop — folders", () => {
  it("rejects dropping a folder into its own descendant", () => {
    const folders = [f("a", null, 0), f("b", "a", 0)];
    expect(computeDrop({ draggedKind: "folder", draggedId: "a", targetKind: "folder", targetId: "b", mode: "into", notes: [], folders })).toBeNull();
  });
  it("reorders a folder after another", () => {
    const folders = [f("a", null, 0), f("b", null, 1)];
    const r = computeDrop({ draggedKind: "folder", draggedId: "a", targetKind: "folder", targetId: "b", mode: "after", notes: [], folders });
    expect(r).toEqual({ kind: "folder", parentId: null, orderedIds: ["b", "a"] });
  });
});

describe("computeDrop — cross-type", () => {
  it("returns null for a note dropped before a folder", () => {
    expect(computeDrop({ draggedKind: "note", draggedId: "a", targetKind: "folder", targetId: "f1", mode: "before", notes: [n("a", null, 0)], folders: [f("f1", null, 0)] })).toBeNull();
  });
});
