import { renderHook, act, waitFor } from "@testing-library/react";
import { describe, it, expect, beforeEach, vi } from "vitest";

const { mockLoad, mockSet } = vi.hoisted(() => ({
  mockLoad: vi.fn<() => Promise<Record<string, string>>>(),
  mockSet: vi.fn<(k: string, v: string) => Promise<void>>(),
}));

vi.mock("../api", () => ({
  api: { settings: { load: mockLoad, set: mockSet } },
}));

import { useSettings } from "./useSettings";

beforeEach(() => {
  vi.clearAllMocks();
  mockLoad.mockResolvedValue({});
  mockSet.mockResolvedValue(undefined);
});

describe("useSettings", () => {
  it("defaults pinnedDisplayMode to flat when empty", async () => {
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.pinnedDisplayMode).toBe("flat"));
  });

  it("loads a stored sections mode", async () => {
    mockLoad.mockResolvedValue({ pinnedDisplayMode: "sections" });
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.pinnedDisplayMode).toBe("sections"));
  });

  it("setSetting updates state and persists", async () => {
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.pinnedDisplayMode).toBe("flat"));
    await act(async () => { await result.current.setSetting("pinnedDisplayMode", "sections"); });
    expect(result.current.settings.pinnedDisplayMode).toBe("sections");
    expect(mockSet).toHaveBeenCalledWith("pinnedDisplayMode", "sections");
  });
});

describe("useSettings — startMinimized", () => {
  it("defaults to false and loads stored true", async () => {
    mockLoad.mockResolvedValue({ startMinimized: "true" });
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.startMinimized).toBe(true));
  });

  it("setSetting persists startMinimized as a string", async () => {
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.startMinimized).toBe(false));
    await act(async () => { await result.current.setSetting("startMinimized", true); });
    expect(result.current.settings.startMinimized).toBe(true);
    expect(mockSet).toHaveBeenCalledWith("startMinimized", "true");
  });
});
