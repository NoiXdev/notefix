import { describe, it, expect } from 'vitest';
import { htmlToText, wordHtml } from './docHtml';

describe('htmlToText', () => {
  it('strips tags and breaks on block elements', () => {
    expect(htmlToText('<p>Hallo</p><p>Welt</p>')).toBe('Hallo\nWelt');
    expect(htmlToText('<h1>Titel</h1><ul><li>a</li><li>b</li></ul>')).toContain('Titel');
    expect(htmlToText('<p>x<br>y</p>')).toBe('x\ny');
  });
  it('empty for empty', () => {
    expect(htmlToText('')).toBe('');
  });
});

describe('wordHtml', () => {
  it('wraps body in an Office-namespaced html doc', () => {
    const out = wordHtml('Titel', '<p>Hi</p>');
    expect(out).toContain('urn:schemas-microsoft-com:office:word');
    expect(out).toContain('<p>Hi</p>');
    expect(out).toContain('<title>Titel</title>');
  });
});
