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
  // The contract: one blank line between blocks is the ordinary paragraph
  // separator; every additional blank line is one empty paragraph. Leading
  // blank lines are empty paragraphs before the first block, extra trailing
  // ones after the last. No `<br>` placeholders in the markdown view.
  it('writes the empty paragraphs before a list as extra blank lines, not <br>', () => {
    const md = htmlToMarkdown('<p>Text</p><p></p><p></p><ul><li><p>Punkt</p></li></ul>', { blankLines: true });
    expect(md).toBe('Text\n\n\n\n- Punkt');
    expect(md).not.toContain('<br>');
  });
  it('round-trips blank lines between paragraphs and a list', () => {
    const html = '<p>Text</p><p></p><p></p><ul><li>Punkt</li></ul>';
    expect(markdownToHtml(htmlToMarkdown(html, { blankLines: true })).replace(/\n/g, '')).toBe(html);
  });
  it('round-trips one and two empty paragraphs between paragraphs', () => {
    for (const html of ['<p>a</p><p></p><p>b</p>', '<p>a</p><p></p><p></p><p>b</p>']) {
      expect(markdownToHtml(htmlToMarkdown(html, { blankLines: true })).replace(/\n/g, '')).toBe(html);
    }
    expect(htmlToMarkdown('<p>a</p><p></p><p>b</p>', { blankLines: true })).toBe('a\n\n\nb');
    expect(htmlToMarkdown('<p>a</p><p></p><p></p><p>b</p>', { blankLines: true })).toBe('a\n\n\n\nb');
  });
  it('round-trips leading and trailing empty paragraphs', () => {
    for (const html of ['<p></p><p>a</p>', '<p></p><p></p><p>a</p>', '<p>a</p><p></p>', '<p>a</p><p></p><p></p>']) {
      expect(markdownToHtml(htmlToMarkdown(html, { blankLines: true })).replace(/\n/g, '')).toBe(html);
    }
    expect(htmlToMarkdown('<p></p><p>a</p>', { blankLines: true })).toBe('\na');
    expect(htmlToMarkdown('<p>a</p><p></p>', { blankLines: true })).toBe('a\n\n\n');
  });
  it('treats <p><br></p> as a blank line too', () => {
    expect(htmlToMarkdown('<p>a</p><p><br></p><p>b</p>', { blankLines: true })).toBe('a\n\n\nb');
  });
  it('turns blank lines typed in the markdown view into empty paragraphs', () => {
    const strip = (md: string) => markdownToHtml(md).replace(/\n/g, '');
    expect(strip('a\n\nb')).toBe('<p>a</p><p>b</p>'); // the ordinary separator
    expect(strip('a\n\n\nb')).toBe('<p>a</p><p></p><p>b</p>');
    expect(strip('a\n\n\n\nb')).toBe('<p>a</p><p></p><p></p><p>b</p>');
    expect(strip('\na')).toBe('<p></p><p>a</p>');
    expect(strip('a\n\n')).toBe('<p>a</p>'); // an editor's trailing newline is not a paragraph
    expect(strip('a\n\n\n')).toBe('<p>a</p><p></p>');
  });
  it('leaves blank lines inside fenced code alone', () => {
    const html = markdownToHtml('x\n\n```\ncode\n\n\nmore\n```\n\n\ny');
    expect(html).toContain('code\n\n\nmore');
    expect(html.replace(/\n/g, '')).toBe('<p>x</p><pre><code>codemore</code></pre><p></p><p>y</p>');
  });
  it('still reads a literal <br> line (older markdown, or typed) as an empty paragraph', () => {
    expect(markdownToHtml('a\n\n<br>\n\nb').replace(/\n/g, '')).toBe('<p>a</p><p></p><p>b</p>');
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

describe('markdown headings', () => {
  it('round-trips every atx level', () => {
    for (const level of [1, 2, 3, 4, 5, 6]) {
      const hashes = '#'.repeat(level);
      const html = markdownToHtml(`${hashes} Titel`);
      expect(html).toContain(`<h${level}`);
      expect(htmlToMarkdown(html)).toBe(`${hashes} Titel`);
    }
  });
});
