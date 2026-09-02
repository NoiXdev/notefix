import { Extension } from '@tiptap/core';
import { Plugin, PluginKey, TextSelection } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';
import type { Node as PMNode } from '@tiptap/pm/model';
import type { Editor } from '@tiptap/react';
import { findMatches } from '../search';

export interface Match { from: number; to: number; }
export interface SearchState { query: string; matches: Match[]; current: number; }
export interface MatchDecorationSpec { from: number; to: number; class: string; }

const searchKey = new PluginKey<SearchState>('search-highlight');

/** Read the current search state (matches count + current index) off an editor. */
export function searchState(editor: Editor): SearchState {
  return searchKey.getState(editor.state) ?? { query: '', matches: [], current: -1 };
}

/** Pure: decoration range + CSS class for every match, marking `current` distinctly.
 *  Consumed by the plugin's `decorations()` prop below. */
export function matchDecorations(matches: Match[], current: number): MatchDecorationSpec[] {
  return matches.map((m, i) => ({
    from: m.from,
    to: m.to,
    class: i === current ? 'search-match search-match-current' : 'search-match',
  }));
}

/** Pure: the next match index when stepping by `step` (±1, but any integer works),
 *  wrapping around the match list. -1 when there are no matches (nothing to step to). */
export function nextMatchIndex(current: number, step: number, length: number): number {
  if (length <= 0) return -1;
  const base = current < 0 ? 0 : current;
  return (((base + step) % length) + length) % length;
}

/** Pure: match ranges across a flat list of (text, doc-position) runs — the
 *  shape you get from walking a ProseMirror doc's text nodes. Positions in the
 *  result are absolute doc positions (run position + in-text offset). */
export function findSearchRanges(runs: Array<{ text: string; pos: number }>, query: string): Match[] {
  const q = query.trim();
  if (!q) return [];
  const out: Match[] = [];
  for (const { text, pos } of runs) {
    for (const [s, e] of findMatches(text, q)) out.push({ from: pos + s, to: pos + e });
  }
  return out;
}

function computeMatches(doc: PMNode, query: string): Match[] {
  const runs: Array<{ text: string; pos: number }> = [];
  doc.descendants((node, pos) => {
    if (node.isText && node.text) runs.push({ text: node.text, pos });
    return true;
  });
  return findSearchRanges(runs, query);
}

declare module '@tiptap/core' {
  interface Commands<ReturnType> {
    searchHighlight: {
      setSearch: (query: string) => ReturnType;
      stepSearch: (step: number) => ReturnType;
      clearSearch: () => ReturnType;
    };
  }
}

export const SearchHighlight = Extension.create({
  name: 'searchHighlight',

  addProseMirrorPlugins() {
    return [
      new Plugin<SearchState>({
        key: searchKey,
        state: {
          init: () => ({ query: '', matches: [], current: -1 }),
          apply(tr, prev) {
            const meta = tr.getMeta(searchKey) as Partial<SearchState> | undefined;
            if (meta) {
              return {
                query: meta.query ?? prev.query,
                matches: meta.matches ?? prev.matches,
                current: meta.current ?? prev.current,
              };
            }
            if (tr.docChanged && prev.query) {
              const matches = computeMatches(tr.doc, prev.query);
              const current = matches.length ? Math.min(Math.max(prev.current, 0), matches.length - 1) : -1;
              return { query: prev.query, matches, current };
            }
            return prev;
          },
        },
        props: {
          decorations(state) {
            const s = searchKey.getState(state);
            if (!s || !s.matches.length) return DecorationSet.empty;
            return DecorationSet.create(
              state.doc,
              matchDecorations(s.matches, s.current).map(d => Decoration.inline(d.from, d.to, { class: d.class })),
            );
          },
        },
      }),
    ];
  },

  addCommands() {
    return {
      setSearch: (query: string) => ({ state, dispatch }) => {
        if (!dispatch) return true;
        const matches = computeMatches(state.doc, query);
        const current = matches.length ? 0 : -1;
        let tr = state.tr.setMeta(searchKey, { query, matches, current });
        if (matches.length) {
          tr = tr.setSelection(TextSelection.create(tr.doc, matches[0].from, matches[0].to)).scrollIntoView();
        }
        dispatch(tr);
        return true;
      },
      stepSearch: (step: number) => ({ state, dispatch }) => {
        const s = searchKey.getState(state);
        if (!s || !s.matches.length) return false;
        const next = nextMatchIndex(s.current, step, s.matches.length);
        const m = s.matches[next];
        if (dispatch) {
          dispatch(
            state.tr
              .setMeta(searchKey, { current: next })
              .setSelection(TextSelection.create(state.doc, m.from, m.to))
              .scrollIntoView(),
          );
        }
        return true;
      },
      clearSearch: () => ({ state, dispatch }) => {
        if (dispatch) dispatch(state.tr.setMeta(searchKey, { query: '', matches: [], current: -1 }));
        return true;
      },
    };
  },
});
