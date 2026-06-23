import { describe, it, expect } from 'vitest';
import { parseDragId, parseDropId, snapLineStyle } from './dndkit';

describe('dndkit', () => {
  it('parseDragId splits kind and id', () => {
    expect(parseDragId('note:abc')).toEqual({ kind: 'note', id: 'abc' });
    expect(parseDragId('folder:xyz')).toEqual({ kind: 'folder', id: 'xyz' });
  });
  it('parseDropId handles row zones and root', () => {
    expect(parseDropId('note:abc:before')).toEqual({ kind: 'note', id: 'abc', mode: 'before' });
    expect(parseDropId('folder:xyz:into')).toEqual({ kind: 'folder', id: 'xyz', mode: 'into' });
    expect(parseDropId('root:into')).toEqual({ kind: 'root', id: null, mode: 'into' });
  });
  it('snapLineStyle gives before/after boxShadow, else empty', () => {
    expect(String(snapLineStyle('before').boxShadow)).toContain('inset 0 3px');
    expect(String(snapLineStyle('after').boxShadow)).toContain('inset 0 -3px');
    expect(snapLineStyle('into')).toEqual({});
    expect(snapLineStyle(null)).toEqual({});
  });
});
