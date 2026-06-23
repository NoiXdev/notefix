import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import { WIDGETS, WIDGET_KEYS } from './dashboardWidgets';
import type { Note } from './types';

const note = (id: string, content: string, updatedAt = 1): Note =>
  ({ id, content, updatedAt, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0 });

describe('dashboardWidgets', () => {
  it('has the four catalog keys', () => {
    expect(WIDGET_KEYS).toEqual(expect.arrayContaining(['recent', 'due', 'stats', 'pinned']));
  });
  it('recent widget lists notes and selecting one calls onSelectNote', () => {
    const onSelectNote = vi.fn();
    render(<>{WIDGETS.recent.render({ notes: [note('a', '<p>Hallo</p>')], stats: null, onSelectNote })}</>);
    fireEvent.click(screen.getByText('Hallo'));
    expect(onSelectNote).toHaveBeenCalledWith('a');
  });
});
