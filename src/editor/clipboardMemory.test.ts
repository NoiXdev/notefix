import { afterEach, describe, it, expect } from 'vitest';
import { Editor } from '@tiptap/core';
import StarterKit from '@tiptap/starter-kit';
import { DOMSerializer } from '@tiptap/pm/model';
import { htmlToMarkdown } from '../markdown';
import { forgetCopy, pasteRemembered, recallCopy, rememberCopy } from './clipboardMemory';

const editors: Editor[] = [];
function make(content: string) {
  const ed = new Editor({ extensions: [StarterKit], content });
  editors.push(ed);
  return ed;
}
afterEach(() => { editors.forEach(e => e.destroy()); editors.length = 0; forgetCopy(); });

const NOTE = '<h1>Heading 1</h1><h2>Heading 2</h2><p></p><p><strong>Bold Text</strong></p><p><em>Italic</em></p><p></p><p>Normal text</p>';

/** What NoteEditor's copy handler does with copyFormat 'md'. */
function cutAllAsMarkdown(ed: Editor): string {
  const { state } = ed;
  const slice = state.doc.slice(0, state.doc.content.size);
  const div = document.createElement('div');
  div.appendChild(DOMSerializer.fromSchema(state.schema).serializeFragment(slice.content));
  const text = htmlToMarkdown(div.innerHTML);
  rememberCopy(text, slice);
  ed.commands.clearContent();
  return text;
}

describe('clipboardMemory', () => {
  it('restores the cut note losslessly when its markdown is pasted back', () => {
    const ed = make(NOTE);
    const before = ed.getHTML();
    const text = cutAllAsMarkdown(ed);
    expect(text).toContain('# Heading 1');
    expect(pasteRemembered(ed.view, text)).toBe(true);
    expect(ed.getHTML()).toBe(before);
  });

  it('matches across Windows line endings', () => {
    const ed = make(NOTE);
    const text = cutAllAsMarkdown(ed);
    expect(recallCopy(text.replace(/\n/g, '\r\n'))).not.toBeNull();
  });

  it('ignores clipboard text it did not write', () => {
    const ed = make(NOTE);
    cutAllAsMarkdown(ed);
    expect(pasteRemembered(ed.view, 'something copied elsewhere')).toBe(false);
  });

  it('keeps the plain-text paste inside a code block', () => {
    const ed = make('<pre><code>x</code></pre>');
    rememberCopy('abc', make('<p>abc</p>').state.doc.slice(1, 4));
    ed.commands.setTextSelection(2);
    expect(pasteRemembered(ed.view, 'abc')).toBe(false);
  });

  it('remembers nothing until a copy happens', () => {
    expect(recallCopy('anything')).toBeNull();
  });
});
