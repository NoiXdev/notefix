import { describe, it, expect } from 'vitest';
import { nextContextId, type ContextInfo } from './contexts';

const ctx = (id: string, active = false): ContextInfo => ({
  id, label: id, kind: 'local', path: '', serverUrl: '', workspaceId: '', active,
});

describe('nextContextId', () => {
  it('returns null with fewer than two contexts', () => {
    expect(nextContextId([])).toBeNull();
    expect(nextContextId([ctx('a', true)])).toBeNull();
  });

  it('cycles forward from the active context', () => {
    expect(nextContextId([ctx('a', true), ctx('b'), ctx('c')])).toBe('b');
    expect(nextContextId([ctx('a'), ctx('b', true), ctx('c')])).toBe('c');
  });

  it('wraps around at the end', () => {
    expect(nextContextId([ctx('a'), ctx('b'), ctx('c', true)])).toBe('a');
  });

  it('starts from the first when none is active', () => {
    expect(nextContextId([ctx('a'), ctx('b')])).toBe('b');
  });
});
