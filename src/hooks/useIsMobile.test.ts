import { renderHook, act } from '@testing-library/react';
import { describe, it, expect, vi, afterEach } from 'vitest';
import { useIsMobile } from './useIsMobile';

function mockMatchMedia(initialMatches: boolean) {
  const listeners = new Set<() => void>();
  const mql = {
    matches: initialMatches,
    media: '',
    addEventListener: vi.fn((_: string, cb: () => void) => { listeners.add(cb); }),
    removeEventListener: vi.fn((_: string, cb: () => void) => { listeners.delete(cb); }),
  };
  const matchMediaFn = vi.fn(() => mql);
  vi.stubGlobal('matchMedia', matchMediaFn);
  return {
    mql,
    matchMediaFn,
    fireChange(next: boolean) {
      mql.matches = next;
      listeners.forEach(cb => cb());
    },
  };
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('useIsMobile', () => {
  it('reflects the initial matchMedia state', () => {
    mockMatchMedia(true);
    const { result } = renderHook(() => useIsMobile());
    expect(result.current).toBe(true);
  });

  it('registers a change listener on mount and updates on change events', () => {
    const { mql, fireChange } = mockMatchMedia(false);
    const { result } = renderHook(() => useIsMobile());
    expect(result.current).toBe(false);
    expect(mql.addEventListener).toHaveBeenCalledWith('change', expect.any(Function));

    act(() => fireChange(true));
    expect(result.current).toBe(true);
  });

  it('removes the change listener on unmount', () => {
    const { mql } = mockMatchMedia(false);
    const { unmount } = renderHook(() => useIsMobile());
    expect(mql.removeEventListener).not.toHaveBeenCalled();
    unmount();
    expect(mql.removeEventListener).toHaveBeenCalledWith('change', expect.any(Function));
  });

  it('uses a custom breakpoint in the media query string', () => {
    const { matchMediaFn } = mockMatchMedia(false);
    renderHook(() => useIsMobile(1024));
    expect(matchMediaFn).toHaveBeenCalledWith('(max-width: 1024px)');
  });

  it('falls back to false and skips the listener when matchMedia is unavailable', () => {
    vi.stubGlobal('matchMedia', undefined);
    const { result } = renderHook(() => useIsMobile());
    expect(result.current).toBe(false);
  });
});
