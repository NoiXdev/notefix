import { describe, it, expect } from 'vitest';
import { parseDragId, parseDropId } from './dndkit';

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
});
