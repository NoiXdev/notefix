import { describe, it, expect } from 'vitest';
import { findMatches } from './search';

describe('findMatches', () => {
  it('returns all non-overlapping ranges, case-insensitive', () => {
    expect(findMatches('aXaxA', 'a')).toEqual([[0, 1], [2, 3], [4, 5]]);
    expect(findMatches('Hello hello', 'hello')).toEqual([[0, 5], [6, 11]]);
  });

  it('returns nothing for an empty query or no match', () => {
    expect(findMatches('abc', '')).toEqual([]);
    expect(findMatches('abc', 'z')).toEqual([]);
  });
});
