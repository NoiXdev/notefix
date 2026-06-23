import { describe, it, expect } from 'vitest';
import { SHORTCUTS } from './shortcuts';
describe('shortcuts', () => {
  it('lists shortcuts incl. new note', () => {
    expect(SHORTCUTS.length).toBeGreaterThan(0);
    expect(SHORTCUTS.some(s => s.description === 'Neue Notiz')).toBe(true);
  });
});
