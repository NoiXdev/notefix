import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import Dashboard from './Dashboard';
import type { Note } from '../types';

const note = (id: string, content: string): Note =>
  ({ id, content, updatedAt: 1, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0 });

const base = {
  notes: [note('a', '<p>Hallo</p>')],
  folders: [],
  stats: { notes: 1, archived: 0, characters: 5, words: 1 },
  onSelectNote: vi.fn(),
  onCreateNote: vi.fn(),
  onChangeLayout: vi.fn(),
  onToggleEdit: vi.fn(),
};

describe('Dashboard', () => {
  it('renders widgets in layout order and selects a recent note', () => {
    render(<Dashboard {...base} layout={[{ key: 'recent', w: 1 }, { key: 'stats', w: 1 }]} editMode={false} />);
    expect(screen.getByText('Zuletzt bearbeitet')).toBeInTheDocument();
    expect(screen.getByText('Statistik')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Hallo'));
    expect(base.onSelectNote).toHaveBeenCalledWith('a');
  });

  it('edit mode removes a widget via onChangeLayout', () => {
    const onChangeLayout = vi.fn();
    render(<Dashboard {...base} onChangeLayout={onChangeLayout} layout={[{ key: 'recent', w: 1 }, { key: 'stats', w: 1 }]} editMode={true} />);
    fireEvent.click(screen.getAllByTitle('Entfernen')[0]);
    expect(onChangeLayout).toHaveBeenCalledWith([{ key: 'stats', w: 1 }]);
  });

  it('edit mode adds an available widget via onChangeLayout', () => {
    const onChangeLayout = vi.fn();
    render(<Dashboard {...base} onChangeLayout={onChangeLayout} layout={[{ key: 'recent', w: 1 }]} editMode={true} />);
    fireEvent.click(screen.getByText('+ Statistik'));
    expect(onChangeLayout).toHaveBeenCalledWith([{ key: 'recent', w: 1 }, { key: 'stats', w: 1 }]);
  });

  it('edit mode width toggle calls onChangeLayout with w=2', () => {
    const onChangeLayout = vi.fn();
    render(<Dashboard {...base} onChangeLayout={onChangeLayout} layout={[{ key: 'recent', w: 1 }]} editMode />);
    fireEvent.click(screen.getByTitle('Breite'));
    expect(onChangeLayout).toHaveBeenCalledWith([{ key: 'recent', w: 2 }]);
  });
});
