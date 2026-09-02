import { renderHook, act, waitFor } from "@testing-library/react";
import { describe, it, expect, beforeEach, vi } from "vitest";
import type { VaultStatus } from "../types";

const { mockStatus, mockSetup, mockUnlock, mockUnlockRecovery, mockUnlockBiometric, mockLock, mockChangePassphrase, contextChangedRef, mockOnContextChanged } = vi.hoisted(() => {
  const contextChangedRef = { current: null as null | (() => void) };
  return {
    mockStatus: vi.fn<() => Promise<VaultStatus>>(),
    mockSetup: vi.fn<(passphrase: string) => Promise<string[]>>(),
    mockUnlock: vi.fn<(passphrase: string) => Promise<void>>(),
    mockUnlockRecovery: vi.fn<(recovery: string) => Promise<void>>(),
    mockUnlockBiometric: vi.fn<() => Promise<void>>(),
    mockLock: vi.fn<() => Promise<void>>(),
    mockChangePassphrase: vi.fn<(current: string, next: string) => Promise<void>>(),
    contextChangedRef,
    mockOnContextChanged: vi.fn<(cb: () => void) => () => void>((cb) => {
      contextChangedRef.current = cb;
      return () => { contextChangedRef.current = null; };
    }),
  };
});

vi.mock("../api", () => ({
  api: {
    onContextChanged: mockOnContextChanged,
    vault: {
      status: mockStatus,
      setup: mockSetup,
      unlock: mockUnlock,
      unlockRecovery: mockUnlockRecovery,
      unlockBiometric: mockUnlockBiometric,
      lock: mockLock,
      changePassphrase: mockChangePassphrase,
    },
  },
}));

import { useVault } from "./useVault";

beforeEach(() => {
  vi.clearAllMocks();
  contextChangedRef.current = null;
  mockOnContextChanged.mockImplementation((cb) => {
    contextChangedRef.current = cb;
    return () => { contextChangedRef.current = null; };
  });
  mockStatus.mockResolvedValue({ exists: true, unlocked: false, biometric: false });
  mockSetup.mockResolvedValue(["code-1", "code-2"]);
  mockUnlock.mockResolvedValue(undefined);
  mockUnlockRecovery.mockResolvedValue(undefined);
  mockUnlockBiometric.mockResolvedValue(undefined);
  mockLock.mockResolvedValue(undefined);
  mockChangePassphrase.mockResolvedValue(undefined);
});

describe("useVault", () => {
  it("fetches status on mount", async () => {
    const { result } = renderHook(() => useVault());
    await waitFor(() => expect(mockStatus).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(result.current.status).toEqual({ exists: true, unlocked: false, biometric: false }));
  });

  it("unlock calls api.vault.unlock then refreshes status", async () => {
    const { result } = renderHook(() => useVault());
    await waitFor(() => expect(result.current.status.exists).toBe(true));

    mockStatus.mockResolvedValue({ exists: true, unlocked: true, biometric: false });
    await act(async () => {
      await result.current.unlock("pw");
    });

    expect(mockUnlock).toHaveBeenCalledWith("pw");
    expect(mockStatus).toHaveBeenCalledTimes(2);
    expect(result.current.status.unlocked).toBe(true);
  });

  it("setup calls api.vault.setup, refreshes status, and returns recovery codes", async () => {
    const { result } = renderHook(() => useVault());
    await waitFor(() => expect(mockStatus).toHaveBeenCalledTimes(1));

    let codes: string[] = [];
    await act(async () => {
      codes = await result.current.setup("pw");
    });

    expect(mockSetup).toHaveBeenCalledWith("pw");
    expect(codes).toEqual(["code-1", "code-2"]);
    expect(mockStatus).toHaveBeenCalledTimes(2);
  });

  it("unlockRecovery calls api.vault.unlockRecovery then refreshes status", async () => {
    const { result } = renderHook(() => useVault());
    await waitFor(() => expect(mockStatus).toHaveBeenCalledTimes(1));

    await act(async () => {
      await result.current.unlockRecovery("recovery-code");
    });

    expect(mockUnlockRecovery).toHaveBeenCalledWith("recovery-code");
    expect(mockStatus).toHaveBeenCalledTimes(2);
  });

  it("unlockBiometric calls api.vault.unlockBiometric then refreshes status", async () => {
    const { result } = renderHook(() => useVault());
    await waitFor(() => expect(mockStatus).toHaveBeenCalledTimes(1));

    await act(async () => {
      await result.current.unlockBiometric();
    });

    expect(mockUnlockBiometric).toHaveBeenCalledTimes(1);
    expect(mockStatus).toHaveBeenCalledTimes(2);
  });

  it("lock calls api.vault.lock then refreshes status", async () => {
    const { result } = renderHook(() => useVault());
    await waitFor(() => expect(mockStatus).toHaveBeenCalledTimes(1));

    await act(async () => {
      await result.current.lock();
    });

    expect(mockLock).toHaveBeenCalledTimes(1);
    expect(mockStatus).toHaveBeenCalledTimes(2);
  });

  it("changePassphrase calls api.vault.changePassphrase then refreshes status", async () => {
    const { result } = renderHook(() => useVault());
    await waitFor(() => expect(mockStatus).toHaveBeenCalledTimes(1));

    await act(async () => {
      await result.current.changePassphrase("old", "new");
    });

    expect(mockChangePassphrase).toHaveBeenCalledWith("old", "new");
    expect(mockStatus).toHaveBeenCalledTimes(2);
  });

  // F3: every page that uses the hook — Security, Contexts, their banners —
  // has to follow a context switch or a backend-side vault change. Owning the
  // subscription here is what makes that true for all of them at once.
  it("refreshes on context-changed, and unsubscribes on unmount", async () => {
    const { unmount } = renderHook(() => useVault());
    await waitFor(() => expect(mockStatus).toHaveBeenCalledTimes(1));
    expect(contextChangedRef.current).not.toBeNull();

    await act(async () => { contextChangedRef.current?.(); });
    expect(mockStatus).toHaveBeenCalledTimes(2);

    unmount();
    expect(contextChangedRef.current).toBeNull();
  });

  it("survives a failing status without an unhandled rejection", async () => {
    mockStatus.mockRejectedValueOnce(new Error("no active context"));
    const { result } = renderHook(() => useVault());
    await waitFor(() => expect(mockStatus).toHaveBeenCalledTimes(1));
    // The default status stands; nothing threw.
    expect(result.current.status.exists).toBe(false);

    mockStatus.mockResolvedValue({ exists: true, unlocked: true, biometric: false } as VaultStatus);
    await act(async () => { await result.current.refresh(); });
    expect(result.current.status.unlocked).toBe(true);
  });
});
