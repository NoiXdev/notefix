import { render, screen } from '@testing-library/react';
import { DndContext } from '@dnd-kit/core';
import type { ReactNode } from 'react';
import { describe, it, expect, vi } from 'vitest';
import NoteRow from './NoteRow';
import type { Note } from '../types';

const note = (o: Partial<Note> = {}): Note =>
  ({ id: 'a', content: '<p>Hi</p>', updatedAt: new Date(2026, 0, 2).getTime(), pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0, ...o });

const wrap = (ui: ReactNode) => render(<DndContext>{ui}</DndContext>);
const props = { depth: 0, selected: false, dropMode: null, dateFormat: 'iso' as const, onSelect: vi.fn(), onDelete: vi.fn(), onContextMenu: vi.fn() };

describe('NoteRow compact & progress', () => {
  it('compact hides the date', () => {
    wrap(<NoteRow {...props} note={note()} compact />);
    expect(screen.getByText('Hi')).toBeInTheDocument();
    expect(screen.queryByText('2026-01-02')).not.toBeInTheDocument();
  });
  it('non-compact shows the date', () => {
    wrap(<NoteRow {...props} note={note()} />);
    expect(screen.getByText('2026-01-02')).toBeInTheDocument();
  });
  it('shows a progress bar when tasks exist and showProgress is on', () => {
    const { container } = wrap(<NoteRow {...props} note={note({ content: '<ul><li data-checked="true">x</li><li data-checked="false">y</li></ul>' })} showProgress />);
    expect(container.querySelector('[style*="width: 50%"]')).toBeTruthy();
  });
});
