import { afterEach, describe, it, expect } from 'vitest';
import { Editor } from '@tiptap/core';
import StarterKit from '@tiptap/starter-kit';
import {
  SearchHighlight,
  searchState,
  findSearchRanges,
  matchDecorations,
  nextMatchIndex,
} from './searchHighlight';

describe('findSearchRanges (pure)', () => {
  it('returns no ranges for an empty query', () => {
    expect(findSearchRanges([{ text: 'hello world', pos: 0 }], '')).toEqual([]);
  });

  it('returns no ranges for a whitespace-only query', () => {
    expect(findSearchRanges([{ text: 'hello world', pos: 0 }], '   ')).toEqual([]);
  });

  it('returns no ranges when there is no match', () => {
    expect(findSearchRanges([{ text: 'hello world', pos: 0 }], 'xyz')).toEqual([]);
  });

  it('finds a single match at the correct offset within one run', () => {
    expect(findSearchRanges([{ text: 'hello world', pos: 0 }], 'world')).toEqual([{ from: 6, to: 11 }]);
  });

  it('adds the run position to produce absolute doc positions', () => {
    // e.g. a text node that starts at doc position 4 (after an opening paragraph tag).
    expect(findSearchRanges([{ text: 'hello world', pos: 4 }], 'world')).toEqual([{ from: 10, to: 15 }]);
  });

  it('finds every non-overlapping occurrence within a run', () => {
    expect(findSearchRanges([{ text: 'aaaa', pos: 0 }], 'aa')).toEqual([
      { from: 0, to: 2 },
      { from: 2, to: 4 },
    ]);
  });

  it('is case-insensitive', () => {
    expect(findSearchRanges([{ text: 'Hello World', pos: 0 }], 'HELLO')).toEqual([{ from: 0, to: 5 }]);
  });

  it('matches unicode text', () => {
    expect(findSearchRanges([{ text: 'café au café', pos: 0 }], 'café')).toEqual([
      { from: 0, to: 4 },
      { from: 8, to: 12 },
    ]);
  });

  it('maps matches across multiple runs with independent offsets', () => {
    const runs = [
      { text: 'foo bar', pos: 0 }, // "foo bar" -> node size 7, next node starts after some doc gap
      { text: 'bar baz', pos: 10 },
    ];
    expect(findSearchRanges(runs, 'bar')).toEqual([
      { from: 4, to: 7 },
      { from: 10, to: 13 },
    ]);
  });

  it('skips empty-text runs without throwing', () => {
    expect(findSearchRanges([{ text: '', pos: 0 }, { text: 'bar', pos: 5 }], 'bar')).toEqual([{ from: 5, to: 8 }]);
  });

  it('trims surrounding whitespace from the query before matching', () => {
    expect(findSearchRanges([{ text: 'hello world', pos: 0 }], '  world  ')).toEqual([{ from: 6, to: 11 }]);
  });
});

describe('matchDecorations (pure)', () => {
  it('returns an empty array for no matches', () => {
    expect(matchDecorations([], -1)).toEqual([]);
  });

  it('marks the current match with the extra current class', () => {
    const matches = [{ from: 0, to: 3 }, { from: 5, to: 8 }];
    expect(matchDecorations(matches, 1)).toEqual([
      { from: 0, to: 3, class: 'search-match' },
      { from: 5, to: 8, class: 'search-match search-match-current' },
    ]);
  });

  it('marks no match as current when current is -1', () => {
    const matches = [{ from: 0, to: 3 }, { from: 5, to: 8 }];
    expect(matchDecorations(matches, -1)).toEqual([
      { from: 0, to: 3, class: 'search-match' },
      { from: 5, to: 8, class: 'search-match' },
    ]);
  });

  it('marks no match as current when current is out of range', () => {
    const matches = [{ from: 0, to: 3 }];
    expect(matchDecorations(matches, 5)).toEqual([{ from: 0, to: 3, class: 'search-match' }]);
  });

  it('preserves from/to exactly, including overlapping-looking adjacent ranges', () => {
    const matches = [{ from: 0, to: 2 }, { from: 2, to: 4 }];
    expect(matchDecorations(matches, 0)).toEqual([
      { from: 0, to: 2, class: 'search-match search-match-current' },
      { from: 2, to: 4, class: 'search-match' },
    ]);
  });
});

describe('nextMatchIndex (pure)', () => {
  it('returns -1 when there are no matches', () => {
    expect(nextMatchIndex(-1, 1, 0)).toBe(-1);
  });

  it('steps from "no current" (-1) forward to index 1 (matches original stepSearch semantics)', () => {
    expect(nextMatchIndex(-1, 1, 3)).toBe(1);
  });

  it('steps forward through the middle of the list', () => {
    expect(nextMatchIndex(1, 1, 5)).toBe(2);
  });

  it('wraps forward past the end back to 0', () => {
    expect(nextMatchIndex(2, 1, 3)).toBe(0);
  });

  it('wraps backward past the start to the last index', () => {
    expect(nextMatchIndex(0, -1, 3)).toBe(2);
  });

  it('steps backward through the middle of the list', () => {
    expect(nextMatchIndex(2, -1, 5)).toBe(1);
  });

  it('handles a step magnitude larger than the list length', () => {
    expect(nextMatchIndex(0, -5, 3)).toBe(1);
  });

  it('handles a single-match list (always stays at 0)', () => {
    expect(nextMatchIndex(0, 1, 1)).toBe(0);
    expect(nextMatchIndex(0, -1, 1)).toBe(0);
  });
});

// --- Plugin wiring: exercised against a real headless TipTap Editor. ---
describe('SearchHighlight extension (real Editor)', () => {
  const editors: Editor[] = [];
  function makeEditor(content: string) {
    const ed = new Editor({ extensions: [StarterKit, SearchHighlight], content });
    editors.push(ed);
    return ed;
  }
  afterEach(() => { editors.forEach(e => e.destroy()); editors.length = 0; });

  it('starts with empty search state', () => {
    const ed = makeEditor('<p>hello world</p>');
    expect(searchState(ed)).toEqual({ query: '', matches: [], current: -1 });
  });

  it('setSearch populates matches, selects the first one, and decorates the dom', () => {
    const ed = makeEditor('<p>hello world hello</p>');
    ed.commands.setSearch('hello');
    const s = searchState(ed);
    expect(s.query).toBe('hello');
    expect(s.matches).toEqual([{ from: 1, to: 6 }, { from: 13, to: 18 }]);
    expect(s.current).toBe(0);
    expect(ed.state.selection.from).toBe(1);
    expect(ed.state.selection.to).toBe(6);
    const matchEls = ed.view.dom.querySelectorAll('.search-match');
    expect(matchEls.length).toBe(2);
    expect(ed.view.dom.querySelectorAll('.search-match-current').length).toBe(1);
  });

  it('setSearch with no matches leaves current at -1 and adds no decorations', () => {
    const ed = makeEditor('<p>hello world</p>');
    ed.commands.setSearch('xyz');
    expect(searchState(ed)).toEqual({ query: 'xyz', matches: [], current: -1 });
    expect(ed.view.dom.querySelectorAll('.search-match').length).toBe(0);
  });

  it('stepSearch(1) advances to the next match and wraps around', () => {
    const ed = makeEditor('<p>aa bb aa bb aa</p>');
    ed.commands.setSearch('aa');
    expect(searchState(ed).current).toBe(0);
    ed.commands.stepSearch(1);
    expect(searchState(ed).current).toBe(1);
    ed.commands.stepSearch(1);
    expect(searchState(ed).current).toBe(2);
    ed.commands.stepSearch(1); // wraps back to 0
    expect(searchState(ed).current).toBe(0);
  });

  it('stepSearch(-1) moves backward and wraps to the last match', () => {
    const ed = makeEditor('<p>aa bb aa</p>');
    ed.commands.setSearch('aa');
    expect(searchState(ed).current).toBe(0);
    ed.commands.stepSearch(-1);
    expect(searchState(ed).current).toBe(1);
  });

  it('stepSearch moves the selection to the new match', () => {
    const ed = makeEditor('<p>aa bb aa</p>');
    ed.commands.setSearch('aa');
    ed.commands.stepSearch(1);
    const s = searchState(ed);
    expect(ed.state.selection.from).toBe(s.matches[1].from);
    expect(ed.state.selection.to).toBe(s.matches[1].to);
  });

  it('stepSearch is a no-op when there are no matches', () => {
    const ed = makeEditor('<p>hello</p>');
    ed.commands.setSearch('xyz');
    const result = ed.commands.stepSearch(1);
    expect(result).toBe(false);
    expect(searchState(ed).current).toBe(-1);
  });

  it('clearSearch resets query, matches, and current, and removes decorations', () => {
    const ed = makeEditor('<p>hello world hello</p>');
    ed.commands.setSearch('hello');
    ed.commands.clearSearch();
    expect(searchState(ed)).toEqual({ query: '', matches: [], current: -1 });
    expect(ed.view.dom.querySelectorAll('.search-match').length).toBe(0);
  });

  it('recomputes matches when the doc changes while a query is active', () => {
    const ed = makeEditor('<p>hello world</p>');
    ed.commands.setSearch('hello');
    expect(searchState(ed).matches.length).toBe(1);
    // Editing the doc so the query no longer matches should clear the matches
    // (docChanged branch recomputes against the *same* stored query).
    ed.commands.setContent('<p>goodbye world</p>');
    expect(searchState(ed).matches).toEqual([]);
    expect(searchState(ed).current).toBe(-1);
  });

  it('keeps a valid current index after a doc change shrinks the match count', () => {
    const ed = makeEditor('<p>hello hello hello</p>');
    ed.commands.setSearch('hello');
    ed.commands.stepSearch(1);
    ed.commands.stepSearch(1); // current = 2 (last match)
    expect(searchState(ed).current).toBe(2);
    ed.commands.setContent('<p>hello</p>'); // only one match remains
    const s = searchState(ed);
    expect(s.matches.length).toBe(1);
    expect(s.current).toBe(0); // clamped back into range
  });
});
