import { describe, it, expect } from 'vitest';
import { highlightMarkdown } from './mdHighlight';

describe('highlightMarkdown', () => {
  it('returns an empty string for empty input', () => {
    expect(highlightMarkdown('')).toBe('');
  });

  it('leaves plain text with no markdown syntax unwrapped', () => {
    expect(highlightMarkdown('plain text')).toBe('plain text');
  });

  it('wraps an ATX heading in hljs-section', () => {
    expect(highlightMarkdown('# Heading')).toBe('<span class="hljs-section"># Heading</span>');
  });

  it('wraps bold syntax in hljs-strong', () => {
    expect(highlightMarkdown('**bold**')).toBe('<span class="hljs-strong">**bold**</span>');
  });

  it('wraps a bullet-list marker in hljs-bullet', () => {
    expect(highlightMarkdown('- item')).toBe('<span class="hljs-bullet">-</span> item');
  });

  it('wraps inline code in hljs-code', () => {
    expect(highlightMarkdown('`code`')).toBe('<span class="hljs-code">`code`</span>');
  });

  it('wraps a markdown link\'s label and url in separate spans', () => {
    expect(highlightMarkdown('[text](http://example.com)')).toBe(
      '[<span class="hljs-string">text</span>](<span class="hljs-link">http://example.com</span>)',
    );
  });

  it('escapes HTML-significant characters in the input', () => {
    expect(highlightMarkdown('<div>')).toBe('&lt;div&gt;');
  });

  it('highlights multiple lines independently, keeping the newline', () => {
    expect(highlightMarkdown('# Title\nplain')).toBe('<span class="hljs-section"># Title</span>\nplain');
  });
});
