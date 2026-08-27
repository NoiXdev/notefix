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

describe("useSettings — dateFormat", () => {
  it("defaults to auto and loads a stored value", async () => {
    mockLoad.mockResolvedValue({ dateFormat: "de" });
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.dateFormat).toBe("de"));
  });
});

describe("useSettings — pinnedScope", () => {
  it("defaults to perFolder and loads global", async () => {
    mockLoad.mockResolvedValue({ pinnedScope: "global" });
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.pinnedScope).toBe("global"));
  });
});

describe("useSettings — sidebarSide", () => {
  it("defaults to left", async () => {
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.sidebarSide).toBe("left"));
  });

  it("loads a stored right value and falls back to left for junk", async () => {
    mockLoad.mockResolvedValue({ sidebarSide: "right" });
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.sidebarSide).toBe("right"));

    mockLoad.mockResolvedValue({ sidebarSide: "bogus" });
    const { result: result2 } = renderHook(() => useSettings());
    await waitFor(() => expect(result2.current.settings.sidebarSide).toBe("left"));
  });

  it("setSetting persists sidebarSide", async () => {
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.sidebarSide).toBe("left"));
    await act(async () => { await result.current.setSetting("sidebarSide", "right"); });
    expect(result.current.settings.sidebarSide).toBe("right");
    expect(mockSet).toHaveBeenCalledWith("sidebarSide", "right");
  });
});

describe("useSettings — vaultLockScope", () => {
  it("defaults to session", async () => {
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.vaultLockScope).toBe("session"));
  });

  it("loads a stored perNote value and falls back to session for junk", async () => {
    mockLoad.mockResolvedValue({ vaultLockScope: "perNote" });
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.vaultLockScope).toBe("perNote"));

    mockLoad.mockResolvedValue({ vaultLockScope: "bogus" });
    const { result: result2 } = renderHook(() => useSettings());
    await waitFor(() => expect(result2.current.settings.vaultLockScope).toBe("session"));
  });

  it("setSetting persists vaultLockScope", async () => {
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.vaultLockScope).toBe("session"));
    await act(async () => { await result.current.setSetting("vaultLockScope", "perNote"); });
    expect(result.current.settings.vaultLockScope).toBe("perNote");
    expect(mockSet).toHaveBeenCalledWith("vaultLockScope", "perNote");
  });
});

describe("useSettings — mcpProtectedAccess", () => {
  it("defaults to off", async () => {
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.mcpProtectedAccess).toBe("off"));
  });

  it("loads a stored read/readwrite value and falls back to off for junk", async () => {
    mockLoad.mockResolvedValue({ mcpProtectedAccess: "read" });
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.mcpProtectedAccess).toBe("read"));

    mockLoad.mockResolvedValue({ mcpProtectedAccess: "readwrite" });
    const { result: result2 } = renderHook(() => useSettings());
    await waitFor(() => expect(result2.current.settings.mcpProtectedAccess).toBe("readwrite"));

    mockLoad.mockResolvedValue({ mcpProtectedAccess: "bogus" });
    const { result: result3 } = renderHook(() => useSettings());
    await waitFor(() => expect(result3.current.settings.mcpProtectedAccess).toBe("off"));
  });

  it("setSetting persists mcpProtectedAccess", async () => {
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.mcpProtectedAccess).toBe("off"));
    await act(async () => { await result.current.setSetting("mcpProtectedAccess", "readwrite"); });
    expect(result.current.settings.mcpProtectedAccess).toBe("readwrite");
    expect(mockSet).toHaveBeenCalledWith("mcpProtectedAccess", "readwrite");
  });
});

describe("useSettings — auto-lock toggles", () => {
  it("autoLockIdle and autoLockOnHide default to true", async () => {
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.autoLockIdle).toBe(true));
    expect(result.current.settings.autoLockOnHide).toBe(true);
  });

  it("loads stored 'false' as false for both toggles", async () => {
    mockLoad.mockResolvedValue({ autoLockIdle: "false", autoLockOnHide: "false" });
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.autoLockIdle).toBe(false));
    expect(result.current.settings.autoLockOnHide).toBe(false);
  });
});

describe("useSettings — lastSeenVersion", () => {
  it("defaults to an empty string", async () => {
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.lastSeenVersion).toBe(""));
  });

  it("loads a stored value", async () => {
    mockLoad.mockResolvedValue({ lastSeenVersion: "0.5.0" });
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.lastSeenVersion).toBe("0.5.0"));
  });

  it("setSetting persists lastSeenVersion", async () => {
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.lastSeenVersion).toBe(""));
    await act(async () => { await result.current.setSetting("lastSeenVersion", "0.6.0"); });
    expect(result.current.settings.lastSeenVersion).toBe("0.6.0");
    expect(mockSet).toHaveBeenCalledWith("lastSeenVersion", "0.6.0");
  });
});

describe("useSettings — whatsNewOnUpdate", () => {
  it("defaults to true", async () => {
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.whatsNewOnUpdate).toBe(true));
  });

  it("loads a stored 'false' as false", async () => {
    mockLoad.mockResolvedValue({ whatsNewOnUpdate: "false" });
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.whatsNewOnUpdate).toBe(false));
  });

  it("setSetting persists whatsNewOnUpdate", async () => {
    const { result } = renderHook(() => useSettings());
    await waitFor(() => expect(result.current.settings.whatsNewOnUpdate).toBe(true));
    await act(async () => { await result.current.setSetting("whatsNewOnUpdate", false); });
    expect(result.current.settings.whatsNewOnUpdate).toBe(false);
    expect(mockSet).toHaveBeenCalledWith("whatsNewOnUpdate", "false");
  });
});
