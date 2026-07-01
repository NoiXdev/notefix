import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import NoteList from './NoteList';
import type { NoteMeta, Folder } from '../types';
import { getPreview } from '../preview';

vi.mock('../export', () => ({ exportSelected: vi.fn() }));
vi.mock('emoji-picker-react', () => ({ default: () => null, Theme: { DARK: 'dark' } }));
// ContextSwitcher (rendered in the header) hits the Tauri-backed api on mount;
// stub it so jsdom doesn't blow up on real invoke/listen calls.
vi.mock('../api', () => ({
  api: {
    contexts: {
      list: vi.fn().mockResolvedValue([]),
      switch: vi.fn(),
      add: vi.fn(),
      syncStatus: vi.fn().mockResolvedValue({ state: 'local', lastSyncedAt: 0, pending: 0 }),
    },
    onContextChanged: () => () => {},
    onSyncStatus: () => () => {},
  },
}));

const note = (id: string, content: string, updatedAt = Date.now(), pinned = false, archived = false, color = '', dueAt: number | null = null, folderId: string | null = null): NoteMeta =>
  ({ id, updatedAt, pinned, archived, color, dueAt, folderId, position: 0, deletedAt: null, preview: getPreview(content), tasksDone: 0, tasksTotal: 0 });

const defaultProps = {
  notes: [],
  folders: [] as Folder[],
  selectedId: null,
  onSelect: vi.fn(),
  onCreate: vi.fn(),
  onDelete: vi.fn(),
  onOpenSettings: vi.fn(),
  onExportNote: vi.fn(),
  onTogglePin: vi.fn(),
  onArchive: vi.fn(),
  onSetColor: vi.fn(),
  onMoveNote: vi.fn(),
};

beforeEach(() => vi.clearAllMocks());

describe('NoteList — empty state', () => {
  it('shows the empty state message when there are no notes', () => {
    render(<NoteList {...defaultProps} />);
    expect(screen.getByText(/noch keine notizen/i)).toBeInTheDocument();
  });

  it('does not render any note buttons when empty', () => {
    render(<NoteList {...defaultProps} />);
    expect(screen.queryByTitle('Notiz löschen')).not.toBeInTheDocument();
  });
});

describe('NoteList — rendering notes', () => {
  it('renders stripped plain-text preview from HTML content', () => {
    render(<NoteList {...defaultProps} notes={[note('1', '<b>Buy milk</b>')]} />);
    expect(screen.getByText('Buy milk')).toBeInTheDocument();
  });

  it('falls back to the untitled label for empty content', () => {
    render(<NoteList {...defaultProps} notes={[note('1', '')]} />);
    expect(screen.getByText('Ohne Titel')).toBeInTheDocument();
  });

  it('truncates content preview to 60 characters', () => {
    const long = 'A'.repeat(80);
    render(<NoteList {...defaultProps} notes={[note('1', `<p>${long}</p>`)]} />);
    expect(screen.getByText('A'.repeat(60))).toBeInTheDocument();
  });

  it('renders one row per note', () => {
    const notes = [note('1', '<p>First</p>'), note('2', '<p>Second</p>'), note('3', '<p>Third</p>')];
    render(<NoteList {...defaultProps} notes={notes} />);
    expect(screen.getAllByTitle('Notiz löschen')).toHaveLength(3);
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
    fireEvent.click(screen.getByTitle('Neue Notiz'));
    expect(defaultProps.onCreate).toHaveBeenCalledOnce();
  });

  it('calls onSelect with the note id when a note is clicked', () => {
    render(<NoteList {...defaultProps} notes={[note('42', '<p>Click me</p>')]} />);
    fireEvent.click(screen.getByText('Click me'));
    expect(defaultProps.onSelect).toHaveBeenCalledWith('42');
  });

  it('calls onDelete with the note id after confirming the delete dialog', () => {
    render(<NoteList {...defaultProps} notes={[note('7', '<p>Delete me</p>')]} />);
    fireEvent.click(screen.getByTitle('Notiz löschen'));
    fireEvent.click(screen.getByText('In Papierkorb'));
    expect(defaultProps.onDelete).toHaveBeenCalledWith('7');
  });

  it('does not call onSelect when the delete button is clicked', () => {
    render(<NoteList {...defaultProps} notes={[note('7', '<p>Note</p>')]} />);
    fireEvent.click(screen.getByTitle('Notiz löschen'));
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
    fireEvent.click(screen.getByTitle('Mehr'));
    fireEvent.click(screen.getByText('Archiv anzeigen'));
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
    const folders = [{ id: 'f1', name: 'Arbeit', parentId: null, position: 1, icon: '', color: '', sort: 'manual' }];
    const notes = [note('a', '<p>InFolder</p>', 1, false, false, '', null, 'f1')];
    render(<NoteList {...defaultProps} folders={folders} notes={notes} />);
    expect(screen.getByText('Arbeit')).toBeInTheDocument();
    expect(screen.queryByText('InFolder')).not.toBeInTheDocument();
    fireEvent.click(screen.getByText('Arbeit'));
    expect(screen.getByText('InFolder')).toBeInTheDocument();
  });

  it("note context menu offers 'Verschieben nach' with the folder", () => {
    const folders = [{ id: 'f1', name: 'Arbeit', parentId: null, position: 1, icon: '', color: '', sort: 'manual' }];
    render(<NoteList {...defaultProps} folders={folders} notes={[note('a', '<p>Root</p>')]} />);
    fireEvent.contextMenu(screen.getByText('Root'));
    expect(screen.getByText('Verschieben nach')).toBeInTheDocument();
  });

  it("folder context menu offers 'Anpassen…' and opens the customizer", () => {
    const folders = [{ id: 'f1', name: 'Arbeit', parentId: null, position: 1, icon: '', color: '', sort: 'manual' }];
    render(<NoteList {...defaultProps} folders={folders} onSetFolderIcon={vi.fn()} onSetFolderColor={vi.fn()} />);
    fireEvent.contextMenu(screen.getByText('Arbeit'));
    fireEvent.click(screen.getByText('Anpassen…'));
    expect(screen.getByText('Font Awesome')).toBeInTheDocument();
  });

  it("renders a custom folder icon", () => {
    const folders = [{ id: 'f1', name: 'Arbeit', parentId: null, position: 1, icon: 'fa:star', color: '', sort: 'manual' }];
    const { container } = render(<NoteList {...defaultProps} folders={folders} />);
    expect(container.querySelector('[data-icon="star"]')).toBeTruthy();
  });

  it("folder context menu offers Sortierung", () => {
    const folders = [{ id: 'f1', name: 'Arbeit', parentId: null, position: 1, icon: '', color: '', sort: 'manual' }];
    render(<NoteList {...defaultProps} folders={folders} onSetFolderSort={vi.fn()} />);
    fireEvent.contextMenu(screen.getByText('Arbeit'));
    expect(screen.getByText('Sortierung')).toBeInTheDocument();
  });
});

describe("NoteList — drag and drop", () => {
  it("note rows are draggable", () => {
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Drag me</p>')]} />);
    expect(screen.getByText('Drag me').closest('[aria-roledescription="draggable"]')).toBeTruthy();
  });

  it("renders notes in position order, not by updatedAt", () => {
    const notes = [
      { id: 'a', preview: 'AAA', tasksDone: 0, tasksTotal: 0, updatedAt: 999, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 5, deletedAt: null },
      { id: 'b', preview: 'BBB', tasksDone: 0, tasksTotal: 0, updatedAt: 1, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 1, deletedAt: null },
    ];
    render(<NoteList {...defaultProps} notes={notes} />);
    const texts = screen.getAllByText(/AAA|BBB/).map(e => e.textContent);
    expect(texts).toEqual(['BBB', 'AAA']); // position 1 before position 5, despite AAA being newer
  });
});

describe("NoteList — header overflow", () => {
  it("the menu offers Dashboard and Einstellungen", () => {
    render(<NoteList {...defaultProps} onOpenDashboard={vi.fn()} />);
    fireEvent.click(screen.getByTitle('Mehr'));
    expect(screen.getByText('Dashboard')).toBeInTheDocument();
    expect(screen.getByText('Einstellungen')).toBeInTheDocument();
  });
});

describe("NoteList — delete & trash", () => {
  it("context menu 'Löschen' opens a confirm dialog (trash wording)", () => {
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Note</p>')]} />);
    fireEvent.contextMenu(screen.getByText('Note'));
    fireEvent.click(screen.getByText('Löschen'));
    expect(screen.getByText('In den Papierkorb verschieben?')).toBeInTheDocument();
  });
  it("confirming delete calls onDelete", () => {
    const onDelete = vi.fn();
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Note</p>')]} onDelete={onDelete} />);
    fireEvent.contextMenu(screen.getByText('Note'));
    fireEvent.click(screen.getByText('Löschen'));
    fireEvent.click(screen.getByText('In Papierkorb'));
    expect(onDelete).toHaveBeenCalledWith('a');
  });
  it("trash view lists trashed notes and restore calls onRestore", () => {
    const onRestore = vi.fn();
    render(<NoteList {...defaultProps} trashed={[note('t', '<p>Weg</p>')]} onRestore={onRestore} />);
    fireEvent.click(screen.getByTitle('Mehr'));
    fireEvent.click(screen.getByText('Papierkorb'));
    expect(screen.getByText('Weg')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Wiederherstellen'));
    expect(onRestore).toHaveBeenCalledWith('t');
  });
});

describe("NoteList — easter egg", () => {
  it("four quick logo clicks open tic-tac-toe", () => {
    render(<NoteList {...defaultProps} />);
    const logo = screen.getByAltText('Notefix');
    for (let i = 0; i < 4; i++) fireEvent.click(logo);
    expect(screen.getByLabelText('Feld 0')).toBeInTheDocument();
  });
});
