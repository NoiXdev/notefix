import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import Dashboard from './Dashboard';
import type { Note } from '../types';

const note = (id: string, content: string): Note =>
  ({ id, content, updatedAt: 1, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0 });

const base = {
  notes: [note('a', '<p>Hallo</p>')],
  stats: { notes: 1, archived: 0, characters: 5, words: 1 },
  onSelectNote: vi.fn(),
  onChangeLayout: vi.fn(),
  onToggleEdit: vi.fn(),
};

describe('Dashboard', () => {
  it('renders widgets in layout order and selects a recent note', () => {
    render(<Dashboard {...base} layout={['recent', 'stats']} editMode={false} />);
    expect(screen.getByText('Zuletzt bearbeitet')).toBeInTheDocument();
    expect(screen.getByText('Statistik')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Hallo'));
    expect(base.onSelectNote).toHaveBeenCalledWith('a');
  });

  it('edit mode removes a widget via onChangeLayout', () => {
    const onChangeLayout = vi.fn();
    render(<Dashboard {...base} onChangeLayout={onChangeLayout} layout={['recent', 'stats']} editMode={true} />);
    fireEvent.click(screen.getAllByTitle('Entfernen')[0]);
    expect(onChangeLayout).toHaveBeenCalledWith(['stats']);
  });

  it('edit mode adds an available widget via onChangeLayout', () => {
    const onChangeLayout = vi.fn();
    render(<Dashboard {...base} onChangeLayout={onChangeLayout} layout={['recent']} editMode={true} />);
    fireEvent.click(screen.getByText('+ Statistik'));
    expect(onChangeLayout).toHaveBeenCalledWith(['recent', 'stats']);
  });
});
