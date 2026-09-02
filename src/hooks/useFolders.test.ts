import { renderHook, act, waitFor } from "@testing-library/react";
import { describe, it, expect, beforeEach, vi } from "vitest";
import type { Folder } from "../types";

const { mockLoad, mockCreate, mockRename, mockMove, mockDelete, mockReorder, mockSetIcon, mockSetColor, mockSetSort, mockOnNotesChanged } = vi.hoisted(() => ({
  mockLoad: vi.fn<() => Promise<Folder[]>>(),
  mockCreate: vi.fn(() => Promise.resolve()),
  mockRename: vi.fn(() => Promise.resolve()),
  mockMove: vi.fn(() => Promise.resolve()),
  mockDelete: vi.fn(() => Promise.resolve()),
  mockReorder: vi.fn(() => Promise.resolve()),
  mockSetIcon: vi.fn(() => Promise.resolve()),
  mockSetColor: vi.fn(() => Promise.resolve()),
  mockSetSort: vi.fn(() => Promise.resolve()),
  mockOnNotesChanged: vi.fn<(cb: () => void) => () => void>(() => () => {}),
}));

vi.mock("../api", () => ({
  api: {
    folders: {
      load: mockLoad,
      create: mockCreate,
      rename: mockRename,
      move: mockMove,
      delete: mockDelete,
      reorder: mockReorder,
      setIcon: mockSetIcon,
      setColor: mockSetColor,
      setSort: mockSetSort,
    },
    onNotesChanged: mockOnNotesChanged,
  },
}));

import { useFolders } from "./useFolders";

beforeEach(() => {
  vi.clearAllMocks();
  mockLoad.mockResolvedValue([]);
  mockOnNotesChanged.mockImplementation(() => () => {});
});

describe("useFolders", () => {
  it("loads folders on mount", async () => {
    mockLoad.mockResolvedValue([{ id: "a", name: "A", parentId: null, position: 1, icon: '', color: '', sort: 'manual' }]);
    const { result } = renderHook(() => useFolders());
    await waitFor(() => expect(result.current.folders).toHaveLength(1));
  });

  it("createFolder generates an id and calls the bridge", async () => {
    const { result } = renderHook(() => useFolders());
    await waitFor(() => expect(mockLoad).toHaveBeenCalled());
    await act(async () => { await result.current.createFolder("Neu", null); });
    expect(mockCreate).toHaveBeenCalledWith(expect.any(String), "Neu", null);
  });

  it("createFolder returns the generated id", async () => {
    const { result } = renderHook(() => useFolders());
    await waitFor(() => expect(mockLoad).toHaveBeenCalled());
    let id = "";
    await act(async () => { id = await result.current.createFolder("Neu", null); });
    expect(id).toEqual(expect.any(String));
    expect(id.length).toBeGreaterThan(0);
  });

  it("renameFolder calls the bridge and reloads", async () => {
    const { result } = renderHook(() => useFolders());
    await waitFor(() => expect(mockLoad).toHaveBeenCalledTimes(1));
    await act(async () => { await result.current.renameFolder("a", "Neuer Name"); });
    expect(mockRename).toHaveBeenCalledWith("a", "Neuer Name");
    expect(mockLoad).toHaveBeenCalledTimes(2);
  });

  it("moveFolder calls the bridge with the new parent and reloads", async () => {
    const { result } = renderHook(() => useFolders());
    await waitFor(() => expect(mockLoad).toHaveBeenCalledTimes(1));
    await act(async () => { await result.current.moveFolder("a", "parent-1"); });
    expect(mockMove).toHaveBeenCalledWith("a", "parent-1");
    expect(mockLoad).toHaveBeenCalledTimes(2);
  });

  it("deleteFolder calls the bridge with the mode and reloads", async () => {
    const { result } = renderHook(() => useFolders());
    await waitFor(() => expect(mockLoad).toHaveBeenCalledTimes(1));
    await act(async () => { await result.current.deleteFolder("a", "recursive"); });
    expect(mockDelete).toHaveBeenCalledWith("a", "recursive");
    expect(mockLoad).toHaveBeenCalledTimes(2);
  });

  it("reorderFolders calls the bridge with parent and ordered ids, then reloads", async () => {
    const { result } = renderHook(() => useFolders());
    await waitFor(() => expect(mockLoad).toHaveBeenCalledTimes(1));
    await act(async () => { await result.current.reorderFolders(null, ["b", "a"]); });
    expect(mockReorder).toHaveBeenCalledWith(null, ["b", "a"]);
    expect(mockLoad).toHaveBeenCalledTimes(2);
  });

  it("setFolderIcon calls the bridge and reloads", async () => {
    const { result } = renderHook(() => useFolders());
    await waitFor(() => expect(mockLoad).toHaveBeenCalledTimes(1));
    await act(async () => { await result.current.setFolderIcon("a", "📁"); });
    expect(mockSetIcon).toHaveBeenCalledWith("a", "📁");
    expect(mockLoad).toHaveBeenCalledTimes(2);
  });

  it("setFolderColor calls the bridge and reloads", async () => {
    const { result } = renderHook(() => useFolders());
    await waitFor(() => expect(mockLoad).toHaveBeenCalledTimes(1));
    await act(async () => { await result.current.setFolderColor("a", "#ff0000"); });
    expect(mockSetColor).toHaveBeenCalledWith("a", "#ff0000");
    expect(mockLoad).toHaveBeenCalledTimes(2);
  });

  it("setFolderSort calls the bridge and reloads", async () => {
    const { result } = renderHook(() => useFolders());
    await waitFor(() => expect(mockLoad).toHaveBeenCalledTimes(1));
    await act(async () => { await result.current.setFolderSort("a", "alpha"); });
    expect(mockSetSort).toHaveBeenCalledWith("a", "alpha");
    expect(mockLoad).toHaveBeenCalledTimes(2);
  });

  it("reload re-fetches the folder list on demand", async () => {
    const { result } = renderHook(() => useFolders());
    await waitFor(() => expect(mockLoad).toHaveBeenCalledTimes(1));
    await act(async () => { await result.current.reload(); });
    expect(mockLoad).toHaveBeenCalledTimes(2);
  });

  it("subscribes to notes-changed on mount and reloads when it fires", async () => {
    let notified: (() => void) | undefined;
    mockOnNotesChanged.mockImplementation((cb: () => void) => { notified = cb; return () => {}; });
    renderHook(() => useFolders());
    await waitFor(() => expect(mockLoad).toHaveBeenCalledTimes(1));
    expect(notified).toBeInstanceOf(Function);
    await act(async () => { notified?.(); });
    await waitFor(() => expect(mockLoad).toHaveBeenCalledTimes(2));
  });

  it("unsubscribes from notes-changed on unmount", async () => {
    const unsubscribe = vi.fn();
    mockOnNotesChanged.mockImplementation(() => unsubscribe);
    const { unmount } = renderHook(() => useFolders());
    await waitFor(() => expect(mockLoad).toHaveBeenCalledTimes(1));
    unmount();
    expect(unsubscribe).toHaveBeenCalled();
  });
});
