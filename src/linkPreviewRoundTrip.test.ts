import { afterEach, describe, it, expect } from 'vitest';
import { Editor } from '@tiptap/core';
import StarterKit from '@tiptap/starter-kit';
import Underline from '@tiptap/extension-underline';
import TaskList from '@tiptap/extension-task-list';
import TaskItem from '@tiptap/extension-task-item';
import { LinkPreview } from './components/LinkPreviewNode';
import { htmlToMarkdown, markdownToHtml } from './markdown';

const editors: Editor[] = [];
// Destroy editors so ProseMirror's DOMObserver timer doesn't fire after teardown.
afterEach(() => { editors.forEach(e => e.destroy()); editors.length = 0; });

// Regression: StarterKit's Link mark (parse priority 1000 at the extension
// level, 50 at the ProseMirror rule level) used to win the `a[href]` parse over
// the linkPreview node, so every markdown round-trip silently downgraded a
// preview into a plain link. LinkPreview's parse rule now carries priority 100.
function makeEditor(content: string) {
  const ed = new Editor({
    extensions: [StarterKit, Underline, TaskList, TaskItem.configure({ nested: true }), LinkPreview],
    content,
  });
  editors.push(ed);
  return ed;
}

describe('link-preview survives a markdown round trip', () => {
  it('parses restored bare-url html into a linkPreview node, not a link mark', () => {
    const ed = makeEditor(markdownToHtml('https://ex.com/a'));
    expect(JSON.stringify(ed.getJSON())).toContain('linkPreview');
    expect(JSON.stringify(ed.getJSON())).not.toContain('"type":"link"');
  });

  it('paste -> getHTML -> markdown -> html -> setContent keeps the preview node', () => {
    const ed = makeEditor('<p></p>');
    const node = ed.state.schema.nodes.linkPreview.create({ href: 'https://ex.com/a', display: 'card' });
    ed.view.dispatch(ed.state.tr.replaceSelectionWith(node));

    const md = htmlToMarkdown(ed.getHTML());
    expect(md).toContain('https://ex.com/a');

    ed.commands.setContent(markdownToHtml(md));
    expect(ed.getHTML()).toContain('ex.com/a');
    expect(JSON.stringify(ed.getJSON())).toContain('linkPreview');
  });

  it('does not turn a labelled markdown link into a preview node', () => {
    const ed = makeEditor(markdownToHtml('[click](https://ex.com/a)'));
    expect(JSON.stringify(ed.getJSON())).not.toContain('linkPreview');
  });
});
