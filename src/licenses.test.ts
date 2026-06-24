import { describe, it, expect } from 'vitest';
import { OSS_LIBS } from './licenses';

describe('OSS_LIBS', () => {
  it('lists a meaningful set of dependencies', () => {
    expect(OSS_LIBS.length).toBeGreaterThanOrEqual(15);
  });

  it('every entry has a name, license and https url', () => {
    for (const lib of OSS_LIBS) {
      expect(lib.name.trim()).not.toBe('');
      expect(lib.license.trim()).not.toBe('');
      expect(lib.url).toMatch(/^https:\/\//);
    }
  });

  it('has no duplicate names', () => {
    const names = OSS_LIBS.map(l => l.name);
    expect(new Set(names).size).toBe(names.length);
  });
});
