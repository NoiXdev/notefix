import { describe, it, expect } from 'vitest';
import { mdCursor, richCounts } from './editorStatus';

describe('mdCursor', () => {
  it('line 1 col 1 at start', () => {
    expect(mdCursor('hello\nworld', 0)).toEqual({ ln: 1, col: 1, length: 11, lines: 2 });
  });
  it('reports line/col from a multi-line offset', () => {
    expect(mdCursor('hello\nworld', 9)).toMatchObject({ ln: 2, col: 4, lines: 2 });
  });
  it('length and lines for 3 lines', () => {
    expect(mdCursor('a\nb\nc', 5)).toMatchObject({ length: 5, lines: 3 });
  });
});

describe('richCounts', () => {
  it('counts words and chars, empty is zero', () => {
    expect(richCounts('', 0)).toEqual({ words: 0, chars: 0, sel: 0 });
    expect(richCounts('hello world', 0)).toEqual({ words: 2, chars: 11, sel: 0 });
  });
  it('reports selection length', () => {
    expect(richCounts('hello', 3)).toMatchObject({ sel: 3 });
  });
});
