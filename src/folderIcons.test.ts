import { describe, it, expect } from 'vitest';
import { searchIcons, FA_BY_NAME, EMOJIS } from './folderIcons';

describe('folderIcons', () => {
  it('search finds star', () => {
    expect(searchIcons('star')).toContain('star');
  });
  it('empty query returns suggestions', () => {
    expect(searchIcons('').length).toBeGreaterThan(0);
  });
  it('caps results at the limit', () => {
    expect(searchIcons('a', 10).length).toBeLessThanOrEqual(10);
  });
  it('FA_BY_NAME maps known names; EMOJIS non-empty', () => {
    expect(FA_BY_NAME['star']).toBeDefined();
    expect(EMOJIS.length).toBeGreaterThan(0);
  });
});
