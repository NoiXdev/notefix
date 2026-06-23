import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import NoteList from './NoteList';
import type { Note, Folder } from '../types';

vi.mock('../export', () => ({ exportSelected: vi.fn() }));

const note = (id: string, content: string, updatedAt = Date.now(), pinned = false, archived = false, color = '', dueAt: number | null = null, folderId: string | null = null): Note =>
  ({ id, content, updatedAt, pinned, archived, color, dueAt, folderId });

const defaultProps = {
  notes: [],
  folders: [] as Folder[],
  selectedId: null,
  onSelect: vi.fn(),
  onCreate: vi.fn(),
  onDelete: vi.fn(),
  onOpenSettings: vi.fn(),
  onTogglePin: vi.fn(),
  onArchive: vi.fn(),
  onSetColor: vi.fn(),
  onMoveNote: vi.fn(),
};

beforeEach(() => vi.clearAllMocks());

describe('NoteList — empty state', () => {
  it('shows the empty state message when there are no notes', () => {
    render(<NoteList {...defaultProps} />);
    expect(screen.getByText(/no notes yet/i)).toBeInTheDocument();
  });

  it('does not render any note buttons when empty', () => {
    render(<NoteList {...defaultProps} />);
    expect(screen.queryByTitle('Delete note')).not.toBeInTheDocument();
  });
});

describe('NoteList — rendering notes', () => {
  it('renders stripped plain-text preview from HTML content', () => {
    render(<NoteList {...defaultProps} notes={[note('1', '<b>Buy milk</b>')]} />);
    expect(screen.getByText('Buy milk')).toBeInTheDocument();
  });

  it('falls back to "New note" for empty content', () => {
    render(<NoteList {...defaultProps} notes={[note('1', '')]} />);
    expect(screen.getByText('New note')).toBeInTheDocument();
  });

  it('truncates content preview to 60 characters', () => {
    const long = 'A'.repeat(80);
    render(<NoteList {...defaultProps} notes={[note('1', `<p>${long}</p>`)]} />);
    expect(screen.getByText('A'.repeat(60))).toBeInTheDocument();
  });

  it('renders one row per note', () => {
    const notes = [note('1', '<p>First</p>'), note('2', '<p>Second</p>'), note('3', '<p>Third</p>')];
    render(<NoteList {...defaultProps} notes={notes} />);
    expect(screen.getAllByTitle('Delete note')).toHaveLength(3);
  });

  it('applies selected style to the active note', () => {
    const notes = [note('1', '<p>Alpha</p>'), note('2', '<p>Beta</p>')];
    render(<NoteList {...defaultProps} notes={notes} selectedId="1" />);

    const alphaBtn = screen.getByText('Alpha').closest('button')!;
    const betaBtn  = screen.getByText('Beta').closest('button')!;

    expect(alphaBtn).toHaveClass('bg-gray-800');
    expect(betaBtn).not.toHaveClass('bg-gray-800');
  });
});

describe('NoteList — interactions', () => {
  it('calls onCreate when the + button is clicked', () => {
    render(<NoteList {...defaultProps} />);
    fireEvent.click(screen.getByTitle('New note'));
    expect(defaultProps.onCreate).toHaveBeenCalledOnce();
  });

  it('calls onSelect with the note id when a note is clicked', () => {
    render(<NoteList {...defaultProps} notes={[note('42', '<p>Click me</p>')]} />);
    fireEvent.click(screen.getByText('Click me'));
    expect(defaultProps.onSelect).toHaveBeenCalledWith('42');
  });

  it('calls onDelete with the note id when the delete button is clicked', () => {
    render(<NoteList {...defaultProps} notes={[note('7', '<p>Delete me</p>')]} />);
    fireEvent.click(screen.getByTitle('Delete note'));
    expect(defaultProps.onDelete).toHaveBeenCalledWith('7');
  });

  it('does not call onSelect when the delete button is clicked', () => {
    render(<NoteList {...defaultProps} notes={[note('7', '<p>Note</p>')]} />);
    fireEvent.click(screen.getByTitle('Delete note'));
    expect(defaultProps.onSelect).not.toHaveBeenCalled();
  });
});

describe("NoteList — pinning", () => {
  it("right-click opens the menu and Anpinnen calls onTogglePin", () => {
    const onTogglePin = vi.fn();
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Note</p>', 1000, false)]} onTogglePin={onTogglePin} />);
    fireEvent.contextMenu(screen.getByText('Note'));
    fireEvent.click(screen.getByText('Anpinnen'));
    expect(onTogglePin).toHaveBeenCalledWith('a', true);
  });

  it("right-click on a pinned note offers Lösen", () => {
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Note</p>', 1000, true)]} />);
    fireEvent.contextMenu(screen.getByText('Note'));
    expect(screen.getByText('Lösen')).toBeInTheDocument();
  });
});

describe("NoteList — color & archive", () => {
  it("uses the note color for the marker", () => {
    render(<NoteList {...defaultProps} notes={[note('a', '<p>X</p>', 1, false, false, '#ef4444')]} />);
    const dot = document.querySelector('[style*="rgb(239, 68, 68)"]');
    expect(dot).toBeTruthy();
  });

  it("archive toggle switches the list to archived notes", () => {
    const notes = [note('a', '<p>Active</p>', 2, false, false), note('b', '<p>Gone</p>', 1, false, true)];
    render(<NoteList {...defaultProps} notes={notes} />);
    expect(screen.getByText('Active')).toBeInTheDocument();
    expect(screen.queryByText('Gone')).not.toBeInTheDocument();
    fireEvent.click(screen.getByTitle('Archiv anzeigen'));
    expect(screen.getByText('Gone')).toBeInTheDocument();
    expect(screen.queryByText('Active')).not.toBeInTheDocument();
  });

  it("context menu offers Archivieren and Exportieren and swatches call onSetColor", () => {
    const onSetColor = vi.fn();
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Note</p>', 1)]} onSetColor={onSetColor} />);
    fireEvent.contextMenu(screen.getByText('Note'));
    expect(screen.getByText('Archivieren')).toBeInTheDocument();
    expect(screen.getByText('Exportieren')).toBeInTheDocument();
    fireEvent.click(screen.getByLabelText('Farbe #22c55e'));
    expect(onSetColor).toHaveBeenCalledWith('a', '#22c55e');
  });
});

describe("NoteList — due date & format", () => {
  it("renders an overdue chip in red", () => {
    render(<NoteList {...defaultProps} notes={[note('a', '<p>X</p>', Date.now(), false, false, '', 1000)]} />);
    const chip = document.querySelector('[style*="rgb(185, 28, 28)"]');
    expect(chip).toBeTruthy();
  });

  it("formats the row date with the dateFormat prop", () => {
    const ts = new Date(2026, 0, 2).getTime();
    render(<NoteList {...defaultProps} notes={[note('a', '<p>X</p>', ts)]} dateFormat="iso" />);
    expect(screen.getByText('2026-01-02')).toBeInTheDocument();
  });
});

describe("NoteList — folders", () => {
  it("renders a folder and reveals its notes when expanded", () => {
    const folders = [{ id: 'f1', name: 'Arbeit', parentId: null, position: 1 }];
    const notes = [note('a', '<p>InFolder</p>', 1, false, false, '', null, 'f1')];
    render(<NoteList {...defaultProps} folders={folders} notes={notes} />);
    expect(screen.getByText('Arbeit')).toBeInTheDocument();
    expect(screen.queryByText('InFolder')).not.toBeInTheDocument();
    fireEvent.click(screen.getByText('Arbeit'));
    expect(screen.getByText('InFolder')).toBeInTheDocument();
  });

  it("note context menu offers 'Verschieben nach' with the folder", () => {
    const folders = [{ id: 'f1', name: 'Arbeit', parentId: null, position: 1 }];
    render(<NoteList {...defaultProps} folders={folders} notes={[note('a', '<p>Root</p>')]} />);
    fireEvent.contextMenu(screen.getByText('Root'));
    expect(screen.getByText('Verschieben nach')).toBeInTheDocument();
  });
});

describe("NoteList — drag and drop", () => {
  it("note rows are draggable", () => {
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Drag me</p>')]} />);
    expect(screen.getByText('Drag me').closest('[draggable="true"]')).toBeTruthy();
  });

  it("dragStart populates dataTransfer so WKWebView actually starts the drag", () => {
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Drag</p>')]} />);
    const setData = vi.fn();
    const row = screen.getByText('Drag').closest('[draggable="true"]')!;
    fireEvent.dragStart(row, { dataTransfer: { setData, effectAllowed: '' } });
    expect(setData).toHaveBeenCalledWith('text/plain', 'a');
  });

  it("renders notes in position order, not by updatedAt", () => {
    const notes = [
      { id: 'a', content: '<p>AAA</p>', updatedAt: 999, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 5 },
      { id: 'b', content: '<p>BBB</p>', updatedAt: 1, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 1 },
    ];
    render(<NoteList {...defaultProps} notes={notes} />);
    const texts = screen.getAllByText(/AAA|BBB/).map(e => e.textContent);
    expect(texts).toEqual(['BBB', 'AAA']); // position 1 before position 5, despite AAA being newer
  });
});
