import { describe, it, expect } from 'vitest';
import { resolveLang } from './lang';
describe('resolveLang', () => {
  it('explicit choice wins', () => {
    expect(resolveLang('de', 'en-US')).toBe('de');
    expect(resolveLang('fr', 'en-US')).toBe('fr');
    expect(resolveLang('en', 'de-DE')).toBe('en');
  });
  it('system matches the OS language, defaults to en', () => {
    expect(resolveLang('system', 'de-DE')).toBe('de');
    expect(resolveLang('system', 'fr-FR')).toBe('fr');
    expect(resolveLang('system', 'es-ES')).toBe('en');
  });
});
