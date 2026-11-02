import type { Slice } from '@tiptap/pm/model';
import type { EditorView } from '@tiptap/pm/view';

/** Copy/cut with a non-rich copy format writes only text/plain (markdown, HTML
 *  source or plain text) so other apps get what the setting asks for. Pasted
 *  back into a note, that text lost its structure: Tiptap's paste rules revived
 *  bold/italic but left `# Heading` literal. So the copied slice is kept in
 *  memory (never persisted — it may be decrypted vault content) and restored
 *  as-is when the clipboard still holds exactly the text we wrote. */
let last: { text: string; slice: Slice } | null = null;

const norm = (s: string) => s.replace(/\r\n?/g, '\n');

export function rememberCopy(text: string, slice: Slice): void {
  last = { text: norm(text), slice };
}

/** The slice copied from a note, if `text` is still what that copy wrote. */
export function recallCopy(text: string): Slice | null {
  return last && last.text === norm(text) ? last.slice : null;
}

export function forgetCopy(): void {
  last = null;
}

/** Paste handler: replaces the selection with the remembered slice when the
 *  clipboard text is ours. Code blocks keep the plain-text paste. */
export function pasteRemembered(view: EditorView, text: string): boolean {
  const slice = recallCopy(text);
  if (!slice || view.state.selection.$from.parent.type.spec.code) return false;
  view.dispatch(view.state.tr.replaceSelection(slice).scrollIntoView().setMeta('paste', true).setMeta('uiEvent', 'paste'));
  return true;
}
