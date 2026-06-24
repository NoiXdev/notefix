import { CodeBlockLowlight } from '@tiptap/extension-code-block-lowlight';
import { createLowlight, common } from 'lowlight';
import type { Editor } from '@tiptap/core';

// Shared highlight.js registry for rich-mode code blocks. `common` covers the
// usual languages (js/ts, html/xml, css, json, bash, python, rust, sql, yaml,
// markdown, …) — enough for note-taking without bloating the bundle further.
export const lowlight = createLowlight(common);

/**
 * Exit a code block on a *double* Enter (two consecutive Enters with no input),
 * which feels more natural than Tiptap's default triple Enter. Returns true when
 * it handled the key (cursor at the end of the block on a trailing blank line),
 * so the keymap can fall through to the normal newline otherwise.
 */
export function codeBlockDoubleEnter(editor: Editor): boolean {
  const { selection } = editor.state;
  const { $from, empty } = selection;
  if (!empty || $from.parent.type.name !== 'codeBlock') return false;
  const isAtEnd = $from.parentOffset === $from.parent.nodeSize - 2;
  const endsWithNewline = $from.parent.textContent.endsWith('\n');
  if (!isAtEnd || !endsWithNewline) return false;
  return editor
    .chain()
    .command(({ tr }) => {
      tr.delete($from.pos - 1, $from.pos); // drop the trailing newline
      return true;
    })
    .exitCode()
    .run();
}

export const CodeBlock = CodeBlockLowlight.extend({
  addKeyboardShortcuts() {
    return {
      ...this.parent?.(),
      Enter: ({ editor }) => codeBlockDoubleEnter(editor),
    };
  },
}).configure({ lowlight, exitOnTripleEnter: false });
