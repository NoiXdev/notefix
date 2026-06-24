import { afterEach, describe, it, expect } from 'vitest';
import { Editor } from '@tiptap/core';
import StarterKit from '@tiptap/starter-kit';
import { CodeBlock, codeBlockDoubleEnter } from './codeBlock';

const editors: Editor[] = [];
function makeEditor(content: string) {
  const ed = new Editor({ extensions: [StarterKit.configure({ codeBlock: false }), CodeBlock], content });
  editors.push(ed);
  ed.commands.focus('end');
  return ed;
}
// Destroy editors so ProseMirror's DOMObserver timer doesn't fire after teardown.
afterEach(() => { editors.forEach(e => e.destroy()); editors.length = 0; });

describe('code block double-enter exit', () => {
  it('exits the code block when at the end on a trailing blank line', () => {
    const ed = makeEditor('<pre><code>foo\n</code></pre>');
    expect(ed.isActive('codeBlock')).toBe(true);
    expect(codeBlockDoubleEnter(ed)).toBe(true);
    expect(ed.isActive('codeBlock')).toBe(false);
  });

  it('does not exit when the last line still has content', () => {
    const ed = makeEditor('<pre><code>foo</code></pre>');
    expect(codeBlockDoubleEnter(ed)).toBe(false);
    expect(ed.isActive('codeBlock')).toBe(true);
  });

  it('does nothing outside a code block', () => {
    const ed = makeEditor('<p>hello</p>');
    expect(codeBlockDoubleEnter(ed)).toBe(false);
  });
});
