import { Extension } from '@tiptap/core';
import { Plugin, PluginKey, TextSelection } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';
import type { Node as PMNode } from '@tiptap/pm/model';
import type { Editor } from '@tiptap/react';
import { findMatches } from '../search';

export interface Match { from: number; to: number; }
export interface SearchState { query: string; matches: Match[]; current: number; }

const searchKey = new PluginKey<SearchState>('search-highlight');

/** Read the current search state (matches count + current index) off an editor. */
export function searchState(editor: Editor): SearchState {
  return searchKey.getState(editor.state) ?? { query: '', matches: [], current: -1 };
}

function computeMatches(doc: PMNode, query: string): Match[] {
  const q = query.trim();
  if (!q) return [];
  const out: Match[] = [];
  doc.descendants((node, pos) => {
    if (node.isText && node.text) {
      for (const [s, e] of findMatches(node.text, q)) out.push({ from: pos + s, to: pos + e });
    }
    return true;
  });
  return out;
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
              s.matches.map((m, i) =>
                Decoration.inline(m.from, m.to, {
                  class: i === s.current ? 'search-match search-match-current' : 'search-match',
                }),
              ),
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
        const next = ((s.current < 0 ? 0 : s.current) + step + s.matches.length) % s.matches.length;
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
