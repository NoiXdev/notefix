import { describe, it, expect } from 'vitest';
import { searchNotes, findMatches, plainText, type SearchItem } from './search';

const item = (id: string, title: string, content: string): SearchItem => ({ id, title, content });

describe('searchNotes', () => {
  it('returns nothing for an empty query', () => {
    expect(searchNotes([item('a', 'Hello', '<p>x</p>')], '  ')).toEqual([]);
  });

  it('matches title and content case-insensitively', () => {
    const items = [item('a', 'Groceries', '<p>milk</p>'), item('b', 'Trip', '<p>Book flights</p>')];
    expect(searchNotes(items, 'MILK').map(r => r.item.id)).toEqual(['a']);
    expect(searchNotes(items, 'flight').map(r => r.item.id)).toEqual(['b']);
  });

  it('ranks title matches before content-only matches', () => {
    const items = [
      item('content', 'Notes', '<p>meeting agenda</p>'),
      item('title', 'Meeting', '<p>nothing</p>'),
    ];
    expect(searchNotes(items, 'meeting').map(r => r.item.id)).toEqual(['title', 'content']);
  });

  it('builds a snippet around the content match', () => {
    const r = searchNotes([item('a', 'T', '<p>the quick brown fox jumps over the lazy dog</p>')], 'fox');
    expect(r[0].snippet).toContain('fox');
  });
});

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

describe('plainText', () => {
  it('strips tags and collapses whitespace', () => {
    expect(plainText('<h1>Hi</h1>\n<p>there</p>')).toBe('Hi there');
  });
});
