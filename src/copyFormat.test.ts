import { describe, it, expect } from 'vitest';
import { selectionToCopy } from './copyFormat';

describe('selectionToCopy', () => {
  it('md → markdown', () => {
    expect(selectionToCopy('<p><strong>Hi</strong></p>', 'md')).toContain('**Hi**');
  });
  it('text → plain text', () => {
    expect(selectionToCopy('<p>Hallo <em>Welt</em></p>', 'text')).toBe('Hallo Welt');
  });
  it('html → unchanged', () => {
    expect(selectionToCopy('<p>x</p>', 'html')).toBe('<p>x</p>');
  });
});
