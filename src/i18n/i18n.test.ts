import { describe, it, expect } from 'vitest';
import { en } from './en';
import { de } from './de';
import { fr } from './fr';

function keys(obj: unknown, prefix = ''): string[] {
  if (!obj || typeof obj !== 'object') return [prefix];
  return Object.entries(obj as Record<string, unknown>).flatMap(([k, v]) => keys(v, prefix ? `${prefix}.${k}` : k));
}

describe('i18n resources', () => {
  it('de and fr have exactly the same keys as en', () => {
    const e = keys(en).sort();
    expect(keys(de).sort()).toEqual(e);
    expect(keys(fr).sort()).toEqual(e);
  });
});
