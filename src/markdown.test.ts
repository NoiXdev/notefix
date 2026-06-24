import { describe, it, expect } from 'vitest';
import { htmlToMarkdown, markdownToHtml } from './markdown';

describe('markdown', () => {
  it('html to markdown for heading/bold/list', () => {
    const md = htmlToMarkdown('<h1>Title</h1><p>a <strong>b</strong></p><ul><li>x</li></ul>');
    expect(md).toContain('# Title');
    expect(md).toContain('**b**');
    expect(md).toContain('- x');
  });
  it('markdown to html', () => {
    const html = markdownToHtml('# Title\n\n- x');
    expect(html).toContain('<h1');
    expect(html).toContain('<li>x</li>');
  });
  it('round-trips bold', () => {
    expect(htmlToMarkdown(markdownToHtml('**hi**'))).toContain('**hi**');
  });
});

describe('markdown task lists', () => {
  it('html task items become - [ ] / - [x]', () => {
    const md = htmlToMarkdown('<ul data-type="taskList"><li data-checked="true">x</li><li data-checked="false">y</li></ul>');
    expect(md).toContain('- [x] x');
    expect(md).toContain('- [ ] y');
  });
  it('markdown checkboxes become a tiptap task list', () => {
    const html = markdownToHtml('- [x] a\n- [ ] b');
    expect(html).toContain('data-type="taskList"');
    expect(html).toContain('data-checked="true"');
    expect(html).toContain('data-checked="false"');
  });
  it('round-trips the checked state', () => {
    const md = htmlToMarkdown('<ul data-type="taskList"><li data-checked="true">done</li></ul>');
    expect(markdownToHtml(md)).toContain('data-checked="true"');
  });
});

describe('markdown link-preview + code block', () => {
  it('a[data-link-preview] becomes the bare url in markdown', () => {
    const html = '<p><a data-link-preview href="https://ex.com/a" data-display="card">Titel</a></p>';
    expect(htmlToMarkdown(html)).toContain('https://ex.com/a');
    expect(htmlToMarkdown(html)).not.toContain('[Titel]');
  });
  it('markdownToHtml restores a bare-url autolink to a link-preview node', () => {
    const out = markdownToHtml('https://ex.com/a');
    expect(out).toContain('data-link-preview');
    expect(out).toContain('https://ex.com/a');
  });
  it('keeps a labelled markdown link as a plain link', () => {
    const out = markdownToHtml('[click](https://ex.com/a)');
    expect(out).not.toContain('data-link-preview');
  });
  it('round-trips a fenced code block', () => {
    const md = htmlToMarkdown('<pre><code>const a = 1;</code></pre>');
    expect(md).toContain('```');
    expect(markdownToHtml(md)).toContain('<code');
  });
});
