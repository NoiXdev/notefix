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
