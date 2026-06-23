import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import type { ReactNode } from 'react';

// react-grid-layout misst Breite/ResizeObserver — in jsdom nicht vorhanden.
// Passthrough-Mock (wie react-select/emoji-picker): rendert nur die Kinder.
vi.mock('react-grid-layout', () => ({
  __esModule: true,
  default: ({ children }: { children: ReactNode }) => children,
  WidthProvider: (C: unknown) => C,
}));

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
  it('renders widgets and selects a recent note', () => {
    render(<Dashboard {...base} layout={[{ key: 'recent', x: 0, y: 0, w: 6, h: 4 }, { key: 'stats', x: 6, y: 0, w: 4, h: 3 }]} editMode={false} />);
    expect(screen.getByText('Zuletzt bearbeitet')).toBeInTheDocument();
    expect(screen.getByText('Statistik')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Hallo'));
    expect(base.onSelectNote).toHaveBeenCalledWith('a');
  });

  it('edit mode removes a widget via onChangeLayout', () => {
    const onChangeLayout = vi.fn();
    render(<Dashboard {...base} onChangeLayout={onChangeLayout} layout={[{ key: 'recent', x: 0, y: 0, w: 6, h: 4 }, { key: 'stats', x: 6, y: 0, w: 4, h: 3 }]} editMode />);
    fireEvent.click(screen.getAllByTitle('Entfernen')[0]);
    expect(onChangeLayout).toHaveBeenCalledWith([{ key: 'stats', x: 6, y: 0, w: 4, h: 3 }]);
  });

  it('edit mode adds an available widget at its default size', () => {
    const onChangeLayout = vi.fn();
    render(<Dashboard {...base} onChangeLayout={onChangeLayout} layout={[{ key: 'recent', x: 0, y: 0, w: 6, h: 4 }]} editMode />);
    fireEvent.click(screen.getByText('+ Statistik'));
    expect(onChangeLayout).toHaveBeenCalledWith([{ key: 'recent', x: 0, y: 0, w: 6, h: 4 }, { key: 'stats', x: 0, y: Infinity, w: 4, h: 3 }]);
  });

  it('shows empty hint when no widgets', () => {
    render(<Dashboard {...base} layout={[]} editMode={false} />);
    expect(screen.getByText(/Keine Widgets/)).toBeInTheDocument();
  });
});
