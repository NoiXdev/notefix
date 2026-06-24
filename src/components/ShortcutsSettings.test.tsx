import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import ShortcutsSettings from './ShortcutsSettings';

describe('ShortcutsSettings', () => {
  it('records a new combo via Ändern + keypress (newNote row)', () => {
    const onChange = vi.fn();
    render(<ShortcutsSettings value={{}} onChange={onChange} />);
    // SHORTCUT_ACTIONS order: navPrev, navNext, newNote, ... → index 2
    fireEvent.click(screen.getAllByText('Ändern')[2]);
    fireEvent.keyDown(window, { key: 'j', metaKey: true });
    expect(onChange).toHaveBeenCalledWith({ newNote: 'Mod+J' });
  });
  it('reset all calls onChange with {}', () => {
    const onChange = vi.fn();
    render(<ShortcutsSettings value={{ newNote: 'Mod+J' }} onChange={onChange} />);
    fireEvent.click(screen.getByText('Alle zurücksetzen'));
    expect(onChange).toHaveBeenCalledWith({});
  });
});
