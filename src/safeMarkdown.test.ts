import { describe, it, expect } from 'vitest';
import { renderMarkdownSafe } from './safeMarkdown';

describe('renderMarkdownSafe', () => {
  it('renders normal markdown formatting', () => {
    const html = renderMarkdownSafe('**bold** and _em_ and a [link](https://example.com)');
    expect(html).toContain('<strong>bold</strong>');
    expect(html).toContain('<em>em</em>');
    expect(html).toContain('<a href="https://example.com">link</a>');
  });

  it('renders headings and lists', () => {
    const html = renderMarkdownSafe('### Added\n- one\n- two');
    expect(html).toContain('<h3>Added</h3>');
    expect(html).toContain('<li>one</li>');
    expect(html).toContain('<li>two</li>');
  });

  it('escapes a raw script tag instead of passing it through', () => {
    const html = renderMarkdownSafe('before <script>alert(1)</script> after');
    expect(html).not.toContain('<script>');
    expect(html).toContain('&lt;script&gt;');
  });

  it('escapes a raw HTML element with an event handler attribute', () => {
    const html = renderMarkdownSafe('<img src=x onerror="alert(1)">');
    expect(html).not.toContain('<img');
    expect(html).toContain('&lt;img');
  });

  it('escapes inline raw HTML mixed into a paragraph', () => {
    const html = renderMarkdownSafe('Click <a href="#" onclick="alert(1)">here</a> now');
    expect(html).not.toContain('<a href="#" onclick');
    expect(html).toContain('&lt;a href=&quot;#&quot; onclick');
  });

  it('blocks a javascript: URL in a markdown link', () => {
    const html = renderMarkdownSafe('[click me](javascript:alert(1))');
    expect(html).not.toContain('javascript:alert');
    expect(html).toContain('href="#"');
  });

  it('blocks a data: URL in a markdown image', () => {
    const html = renderMarkdownSafe('![x](data:text/html,<script>alert(1)</script>)');
    expect(html).not.toContain('data:text/html');
  });

  it('allows normal http(s) and mailto links through unchanged', () => {
    const html = renderMarkdownSafe('[a](https://example.com) [b](mailto:x@example.com)');
    expect(html).toContain('href="https://example.com"');
    expect(html).toContain('href="mailto:x@example.com"');
  });
});
