import { renderHook, act, waitFor } from "@testing-library/react";
import { describe, it, expect, beforeEach, vi } from "vitest";
import type { Folder } from "../types";

const { mockLoad, mockCreate } = vi.hoisted(() => ({
  mockLoad: vi.fn<() => Promise<Folder[]>>(),
  mockCreate: vi.fn(() => Promise.resolve()),
}));

vi.mock("../api", () => ({
  api: {
    folders: { load: mockLoad, create: mockCreate, rename: vi.fn(), move: vi.fn(), delete: vi.fn() },
    onNotesChanged: () => () => {},
  },
}));

import { useFolders } from "./useFolders";

beforeEach(() => {
  vi.clearAllMocks();
  mockLoad.mockResolvedValue([]);
});

describe("useFolders", () => {
  it("loads folders on mount", async () => {
    mockLoad.mockResolvedValue([{ id: "a", name: "A", parentId: null, position: 1 }]);
    const { result } = renderHook(() => useFolders());
    await waitFor(() => expect(result.current.folders).toHaveLength(1));
  });

  it("createFolder generates an id and calls the bridge", async () => {
    const { result } = renderHook(() => useFolders());
    await waitFor(() => expect(mockLoad).toHaveBeenCalled());
    await act(async () => { await result.current.createFolder("Neu", null); });
    expect(mockCreate).toHaveBeenCalledWith(expect.any(String), "Neu", null);
  });
});
