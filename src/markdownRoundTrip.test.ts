import { afterEach, describe, it, expect } from 'vitest';
import { Editor } from '@tiptap/core';
import StarterKit from '@tiptap/starter-kit';
import TaskList from '@tiptap/extension-task-list';
import TaskItem from '@tiptap/extension-task-item';
import { htmlToMarkdown, markdownToHtml } from './markdown';

const editors: Editor[] = [];
// Destroy editors so ProseMirror's DOMObserver timer doesn't fire after teardown.
afterEach(() => { editors.forEach(e => e.destroy()); editors.length = 0; });

function makeEditor(content = '<p></p>') {
  const ed = new Editor({
    extensions: [StarterKit, TaskList, TaskItem.configure({ nested: true })],
    content,
  });
  editors.push(ed);
  return ed;
}

/** Types text the way the user does, so input rules (`# ` -> heading) run. */
function type(ed: Editor, text: string) {
  for (const ch of text) {
    const { from, to } = ed.state.selection;
    const handled = ed.view.someProp('handleTextInput', f => f(ed.view, from, to, ch));
    if (!handled) ed.view.dispatch(ed.state.tr.insertText(ch, from, to));
  }
}

/** The markdown-view toggle in NoteEditor: rich -> markdown -> rich. */
function throughMarkdownView(ed: Editor): string {
  const md = htmlToMarkdown(ed.getHTML(), { blankLines: true });
  ed.commands.setContent(markdownToHtml(md));
  return ed.getHTML();
}

describe('blank lines survive the markdown view', () => {
  it('keeps the empty paragraphs typed before a list', () => {
    // Regression: markdown collapses a run of blank lines, so toggling the
    // markdown view dropped every empty paragraph the user had typed.
    const ed = makeEditor('<p>Text</p><p></p><p></p><ul><li>Punkt</li></ul>');
    const before = ed.getHTML();
    expect(before).toContain('<p></p><p></p>');
    // StarterKit's trailingNode adds the paragraph after the list, so compare
    // against the content with that trailing paragraph allowed.
    expect(throughMarkdownView(ed).replace(/(<p><\/p>)+$/, '')).toBe(before);
  });

  it('keeps blank lines between paragraphs', () => {
    const ed = makeEditor('<p>a</p><p></p><p></p><p></p><p>b</p>');
    expect(throughMarkdownView(ed)).toBe('<p>a</p><p></p><p></p><p></p><p>b</p>');
  });

  it('shows the empty paragraphs as plain blank lines in the markdown view, not <br>', () => {
    const ed = makeEditor('<p>a</p><p></p><p></p><p>b</p>');
    const md = htmlToMarkdown(ed.getHTML(), { blankLines: true });
    expect(md).toBe('a\n\n\n\nb');
    expect(md).not.toContain('<br>');
  });

  it('leaves an empty note empty', () => {
    const ed = makeEditor('<p></p>');
    expect(throughMarkdownView(ed)).toBe('<p></p>');
  });
});

describe('markdown heading syntax in the rich editor', () => {
  it.each([1, 2, 3, 4, 5, 6])('turns %s hashes plus space into that heading', level => {
    const ed = makeEditor();
    type(ed, `${'#'.repeat(level)} Titel`);
    expect(ed.getHTML()).toContain(`<h${level}>Titel</h${level}>`);
  });

  it('keeps a typed heading across the markdown view', () => {
    const ed = makeEditor();
    type(ed, '#### Vier');
    expect(htmlToMarkdown(ed.getHTML())).toContain('#### Vier');
    expect(throughMarkdownView(ed)).toContain('<h4>Vier</h4>');
  });
});
