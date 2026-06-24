import { describe, it, expect } from 'vitest';
import { eventToCombo, matchesCombo, comboLabel, resolveBindings, parseShortcuts } from './shortcuts';

const ev = (init: Partial<KeyboardEvent>): KeyboardEvent =>
  ({ key: '', metaKey: false, ctrlKey: false, shiftKey: false, altKey: false, ...init } as KeyboardEvent);

describe('shortcuts helpers', () => {
  it('eventToCombo builds modifier combos and ignores lone modifiers', () => {
    expect(eventToCombo(ev({ key: 'n', metaKey: true }))).toBe('Mod+N');
    expect(eventToCombo(ev({ key: 'N', metaKey: true, shiftKey: true }))).toBe('Mod+Shift+N');
    expect(eventToCombo(ev({ key: 'ArrowUp' }))).toBe('ArrowUp');
    expect(eventToCombo(ev({ key: 'Escape' }))).toBe('Escape');
    expect(eventToCombo(ev({ key: 'Shift', shiftKey: true }))).toBeNull();
  });
  it('matchesCombo compares meta or ctrl as Mod', () => {
    expect(matchesCombo(ev({ key: 'e', ctrlKey: true }), 'Mod+E')).toBe(true);
    expect(matchesCombo(ev({ key: 'e' }), 'Mod+E')).toBe(false);
  });
  it('comboLabel renders arrows', () => {
    expect(comboLabel('ArrowUp')).toBe('↑');
    expect(comboLabel('Mod+Shift+N')).toBe('Mod + Shift + N');
  });
  it('resolveBindings applies overrides over defaults', () => {
    const b = resolveBindings({ newNote: 'Mod+J' });
    expect(b.newNote).toBe('Mod+J');
    expect(b.archive).toBe('Mod+E');
  });
  it('parseShortcuts keeps known ids, drops junk', () => {
    expect(parseShortcuts(JSON.stringify({ newNote: 'Mod+J', bogus: 'X' }))).toEqual({ newNote: 'Mod+J' });
    expect(parseShortcuts('nope')).toEqual({});
    expect(parseShortcuts(undefined)).toEqual({});
  });
});
