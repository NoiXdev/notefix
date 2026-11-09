import { render, screen } from '@testing-library/react';
import { DndContext } from '@dnd-kit/core';
import type { ReactNode } from 'react';
import { describe, it, expect, vi } from 'vitest';
import NoteRow from './NoteRow';
import type { NoteMeta } from '../types';

const note = (o: Partial<NoteMeta> = {}): NoteMeta =>
  ({ id: 'a', preview: 'Hi', tasksDone: 0, tasksTotal: 0, updatedAt: new Date(2026, 0, 2).getTime(), pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0, deletedAt: null, ...o });

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
    const { container } = wrap(<NoteRow {...props} note={note({ tasksDone: 1, tasksTotal: 2 })} showProgress />);
    expect(container.querySelector('[style*="width: 50%"]')).toBeTruthy();
  });
});

describe('NoteRow sync indicator', () => {
  it('flags a note with changes not yet pushed', () => {
    wrap(<NoteRow {...props} note={note({ dirty: true })} syncIndicator />);
    expect(screen.getByRole('img', { name: 'Noch nicht synchronisiert' })).toBeInTheDocument();
  });
  it('marks a pushed note as up to date', () => {
    wrap(<NoteRow {...props} note={note({ dirty: false })} syncIndicator />);
    expect(screen.getByRole('img', { name: 'Synchronisiert' })).toBeInTheDocument();
  });
  it('shows nothing outside a syncing context', () => {
    wrap(<NoteRow {...props} note={note({ dirty: true })} />);
    expect(screen.queryByRole('img', { name: /synchronisiert/i })).not.toBeInTheDocument();
  });
});
