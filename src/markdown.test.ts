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

describe('markdown blank lines', () => {
  it('keeps the empty paragraphs before a list as explicit blank lines', () => {
    const md = htmlToMarkdown('<p>Text</p><p></p><p></p><ul><li><p>Punkt</p></li></ul>', { blankLines: true });
    expect(md).toBe('Text\n\n<br>\n\n<br>\n\n- Punkt');
  });
  it('round-trips blank lines between paragraphs and a list', () => {
    const html = '<p>Text</p><p></p><p></p><ul><li>Punkt</li></ul>';
    expect(markdownToHtml(htmlToMarkdown(html, { blankLines: true })).replace(/\n/g, '')).toBe(html);
  });
  it('treats <p><br></p> as a blank line too', () => {
    expect(htmlToMarkdown('<p>a</p><p><br></p><p>b</p>', { blankLines: true })).toBe('a\n\n<br>\n\nb');
  });
  it('leaves a note that is only empty paragraphs empty', () => {
    expect(htmlToMarkdown('<p></p>', { blankLines: true })).toBe('');
    expect(htmlToMarkdown('<p></p><p></p>', { blankLines: true })).toBe('');
  });
  it('keeps single line breaks inside a paragraph out of it', () => {
    expect(htmlToMarkdown('<p>a<br>b</p>', { blankLines: true })).not.toContain('<br>');
  });
  it('stays out of exports and copies unless asked for', () => {
    expect(htmlToMarkdown('<p>a</p><p></p><p>b</p>')).toBe('a\n\nb');
  });
  it('does not touch empty paragraphs nested in a list item', () => {
    expect(htmlToMarkdown('<ul><li><p>a</p><p></p></li></ul>', { blankLines: true })).not.toContain('<br>');
  });
});
