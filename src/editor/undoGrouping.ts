import { Extension, type Editor } from '@tiptap/core';
import { closeHistory } from '@tiptap/pm/history';
import type { Node as PMNode } from '@tiptap/pm/model';
import type { Transaction } from '@tiptap/pm/state';
import { ReplaceStep } from '@tiptap/pm/transform';

/** Pure: does inserting `text` at `pos` of `doc` start a new word? True when
 *  the text opens with a non-space character and lands at the start of a
 *  textblock or right after whitespace. */
export function startsWord(doc: PMNode, pos: number, text: string): boolean {
  if (!/^\S/.test(text)) return false;
  const $pos = doc.resolve(pos);
  if (!$pos.parent.isTextblock) return false;
  const off = $pos.parentOffset;
  if (off === 0) return true;
  return /\s/.test($pos.parent.textBetween(off - 1, off, undefined, '￼'));
}

/** Pure: true for a plain typed-text insertion (one ReplaceStep that inserts
 *  inline text without replacing anything) beginning a new word. */
export function opensWord(tr: Transaction): boolean {
  if (tr.steps.length !== 1) return false;
  const step = tr.steps[0];
  if (!(step instanceof ReplaceStep) || step.from !== step.to) return false;
  const { content } = step.slice;
  if (content.childCount === 0 || !content.firstChild?.isText) return false;
  return startsWord(tr.before, step.from, content.textBetween(0, content.size));
}

/** ProseMirror's history merges every adjacent change typed less than 500ms
 *  apart into one undo step, so a fast typist's whole paragraph vanishes with
 *  a single Ctrl+Z. This closes the undo group at each word start, so undo
 *  steps back word by word (pauses still split groups as before). */
export const UndoGrouping = Extension.create({
  name: 'undoGrouping',

  dispatchTransaction({ transaction, next }) {
    // Mid-IME composition the history plugin groups the composed text itself.
    if (!this.editor.view?.composing && opensWord(transaction)) closeHistory(transaction);
    next(transaction);
  },
});

/** Drops the undo/redo stack, keeping every other plugin's state. Used after
 *  replacing the document wholesale (loading a note, applying a remote edit)
 *  so Ctrl+Z can never step back into another note's content. */
export function clearUndoHistory(editor: Editor): void {
  const state = editor.state;
  if (!state?.plugins || typeof editor.view?.updateState !== 'function') return;
  // `reconfigure` keeps the state of plugins present before and after, so a
  // round trip without the history plugin re-initialises only that one.
  const withoutHistory = state.reconfigure({
    plugins: state.plugins.filter((p) => !(p as unknown as { key: string }).key.startsWith('history$')),
  });
  editor.view.updateState(withoutHistory.reconfigure({ plugins: state.plugins }));
}
