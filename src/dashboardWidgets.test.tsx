import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import { WIDGETS, WIDGET_KEYS } from './dashboardWidgets';
import type { NoteMeta, Folder } from './types';
import { getPreview } from './preview';

const note = (id: string, content: string, updatedAt = 1): NoteMeta =>
  ({ id, preview: getPreview(content), tasksDone: 0, tasksTotal: 0, updatedAt, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0, deletedAt: null });

describe('dashboardWidgets', () => {
  it('has the four catalog keys', () => {
    expect(WIDGET_KEYS).toEqual(expect.arrayContaining(['recent', 'due', 'stats', 'pinned']));
  });
  it('recent widget lists notes and selecting one calls onSelectNote', () => {
    const onSelectNote = vi.fn();
    render(<>{WIDGETS.recent.render({ notes: [note('a', '<p>Hallo</p>')], folders: [], stats: null, onSelectNote, onCreateNote: vi.fn() })}</>);
    fireEvent.click(screen.getByText('Hallo'));
    expect(onSelectNote).toHaveBeenCalledWith('a');
  });
});

const folder = (id: string, name: string): Folder => ({ id, name, parentId: null, position: 0, icon: '', color: '', sort: 'manual' });

describe('dashboardWidgets — new', () => {
  it('folders widget lists a folder with its note count', () => {
    render(<>{WIDGETS.folders.render({ notes: [note('a', '<p>x</p>')], folders: [folder('f1', 'Arbeit')], stats: null, onSelectNote: vi.fn(), onCreateNote: vi.fn() } as never)}</>);
    expect(screen.getByText('Arbeit')).toBeInTheDocument();
  });
  it('quicknote widget button calls onCreateNote', () => {
    const onCreateNote = vi.fn();
    render(<>{WIDGETS.quicknote.render({ notes: [], folders: [], stats: null, onSelectNote: vi.fn(), onCreateNote } as never)}</>);
    fireEvent.click(screen.getByText(/Neue Notiz/));
    expect(onCreateNote).toHaveBeenCalled();
  });
  it('calendar widget renders the current month', () => {
    const ctx = { notes: [], folders: [], stats: null, onSelectNote: vi.fn(), onCreateNote: vi.fn() };
    render(<>{WIDGETS.calendar.render(ctx)}</>);
    const month = new Date().toLocaleDateString(undefined, { month: 'long', year: 'numeric' });
    expect(screen.getByText(month)).toBeInTheDocument();
  });
});
