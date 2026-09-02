import { render, screen, fireEvent, act } from '@testing-library/react';
import { describe, it, expect, vi, afterEach } from 'vitest';
import { WIDGETS, WIDGET_KEYS } from './dashboardWidgets';
import type { NoteMeta, Folder, Stats } from './types';
import { getPreview } from './preview';

const note = (id: string, content: string, updatedAt = 1, extra: Partial<NoteMeta> = {}): NoteMeta =>
  ({ id, preview: getPreview(content), tasksDone: 0, tasksTotal: 0, updatedAt, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0, deletedAt: null, ...extra } as NoteMeta);

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

  it('due widget lists notes with a due date, soonest first, and skips notes without one', () => {
    const onSelectNote = vi.fn();
    const withDue = note('a', '<p>Bald</p>', 1, { dueAt: Date.now() + 100000 });
    const soonest = note('b', '<p>Zuerst</p>', 1, { dueAt: Date.now() - 100000 });
    const noDue = note('c', '<p>Kein Datum</p>', 1, { dueAt: null });
    const ctx = { notes: [withDue, soonest, noDue], folders: [], stats: null, onSelectNote, onCreateNote: vi.fn() };
    render(<>{WIDGETS.due.render(ctx as never)}</>);
    expect(screen.queryByText('Kein Datum')).not.toBeInTheDocument();
    const buttons = screen.getAllByRole('button');
    expect(buttons[0]).toHaveTextContent('Zuerst');
    fireEvent.click(screen.getByText('Bald'));
    expect(onSelectNote).toHaveBeenCalledWith('a');
  });

  it('due widget shows an empty-state message when nothing has a due date', () => {
    const ctx = { notes: [note('a', '<p>x</p>', 1, { dueAt: null })], folders: [], stats: null, onSelectNote: vi.fn(), onCreateNote: vi.fn() };
    render(<>{WIDGETS.due.render(ctx as never)}</>);
    expect(screen.getByText(/keine|fällig/i)).toBeInTheDocument();
  });

  it('stats widget renders the note/archived/character/word counts', () => {
    const stats: Stats = { notes: 12, archived: 3, characters: 456, words: 78 };
    const ctx = { notes: [], folders: [], stats, onSelectNote: vi.fn(), onCreateNote: vi.fn() };
    render(<>{WIDGETS.stats.render(ctx)}</>);
    expect(screen.getByText('12')).toBeInTheDocument();
    expect(screen.getByText('3')).toBeInTheDocument();
    expect(screen.getByText('456')).toBeInTheDocument();
    expect(screen.getByText('78')).toBeInTheDocument();
  });

  it('pinned widget lists only pinned, non-archived notes and selecting one calls onSelectNote', () => {
    const onSelectNote = vi.fn();
    const pinned = note('a', '<p>Angeheftet</p>', 1, { pinned: true });
    const unpinned = note('b', '<p>Normal</p>', 1, { pinned: false });
    const ctx = { notes: [pinned, unpinned], folders: [], stats: null, onSelectNote, onCreateNote: vi.fn() };
    render(<>{WIDGETS.pinned.render(ctx as never)}</>);
    expect(screen.getByText('Angeheftet')).toBeInTheDocument();
    expect(screen.queryByText('Normal')).not.toBeInTheDocument();
    fireEvent.click(screen.getByText('Angeheftet'));
    expect(onSelectNote).toHaveBeenCalledWith('a');
  });

  it('pinned widget shows an empty-state message when nothing is pinned', () => {
    const ctx = { notes: [note('a', '<p>x</p>', 1, { pinned: false })], folders: [], stats: null, onSelectNote: vi.fn(), onCreateNote: vi.fn() };
    render(<>{WIDGETS.pinned.render(ctx as never)}</>);
    expect(screen.getByText(/angepinnt/i)).toBeInTheDocument();
  });
});

describe('dashboardWidgets — clock/date', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('clock widget shows the current time and ticks every second', () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date('2026-01-01T10:20:30'));
    const ctx = { notes: [], folders: [], stats: null, onSelectNote: vi.fn(), onCreateNote: vi.fn() };
    render(<>{WIDGETS.clock.render(ctx)}</>);
    expect(screen.getByText(new Date('2026-01-01T10:20:30').toLocaleTimeString())).toBeInTheDocument();

    act(() => { vi.advanceTimersByTime(1000); });
    expect(screen.getByText(new Date('2026-01-01T10:20:31').toLocaleTimeString())).toBeInTheDocument();
  });

  it('clock widget clears its interval on unmount', () => {
    vi.useFakeTimers();
    const clearSpy = vi.spyOn(global, 'clearInterval');
    const ctx = { notes: [], folders: [], stats: null, onSelectNote: vi.fn(), onCreateNote: vi.fn() };
    const { unmount } = render(<>{WIDGETS.clock.render(ctx)}</>);
    unmount();
    expect(clearSpy).toHaveBeenCalled();
    clearSpy.mockRestore();
  });

  it('date widget shows the weekday and the localized date', () => {
    const fixed = new Date('2026-03-04T00:00:00');
    vi.useFakeTimers();
    vi.setSystemTime(fixed);
    const ctx = { notes: [], folders: [], stats: null, onSelectNote: vi.fn(), onCreateNote: vi.fn() };
    render(<>{WIDGETS.date.render(ctx)}</>);
    expect(screen.getByText(fixed.toLocaleDateString(undefined, { weekday: 'long' }))).toBeInTheDocument();
    expect(screen.getByText(fixed.toLocaleDateString())).toBeInTheDocument();
  });
});
