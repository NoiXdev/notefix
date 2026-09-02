import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { ReactNode } from 'react';
import NoteList from './NoteList';
import type { NoteMeta, Folder } from '../types';
import { getPreview } from '../preview';

vi.mock('../export', () => ({ exportSelected: vi.fn() }));
vi.mock('emoji-picker-react', () => ({ default: () => null, Theme: { DARK: 'dark' } }));

// DndContext from @dnd-kit/core needs real pointer/geometry APIs jsdom doesn't
// provide, so drag gestures can't be simulated via fireEvent. Instead capture
// the onDragStart/onDragOver/onDragEnd/onDragCancel callbacks NoteList passes
// to DndContext and invoke them directly — same events dnd-kit would deliver,
// without needing an actual pointer drag. useDraggable/useDroppable stay real
// (they degrade gracefully to inert defaults outside a live drag). DragOverlay
// is also stubbed to just render its children: the real one only renders when
// dnd-kit's own (bypassed) internal context reports an active drag, but the
// content we want to verify is NoteList's own `activeDrag ? (...) : null`
// conditional passed in as those children, not dnd-kit's gating.
const dndRef = vi.hoisted(() => ({ current: null as unknown as { onDragStart: (e: unknown) => void; onDragOver: (e: unknown) => void; onDragEnd: (e: unknown) => void; onDragCancel: () => void } }));
vi.mock('@dnd-kit/core', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@dnd-kit/core')>();
  return {
    ...actual,
    DndContext: (props: { children?: ReactNode } & Record<string, unknown>) => {
      dndRef.current = props as unknown as typeof dndRef.current;
      return props.children;
    },
    DragOverlay: (props: { children?: ReactNode }) => props.children ?? null,
  };
});
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

const note = (id: string, content: string, updatedAt = Date.now(), pinned = false, archived = false, color = '', dueAt: number | null = null, folderId: string | null = null, protectedNote = false, title = ''): NoteMeta =>
  ({ id, updatedAt, pinned, archived, color, dueAt, folderId, position: 0, deletedAt: null, preview: getPreview(content), tasksDone: 0, tasksTotal: 0, protected: protectedNote, title });

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

describe("NoteList — protected notes", () => {
  it("renders the note's plaintext title, not the raw preview, for a protected note", () => {
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Secret</p>', 1000, false, false, '', null, null, true, 'My Secret Note')]} />);
    expect(screen.getByText('My Secret Note')).toBeInTheDocument();
    expect(screen.queryByText('Secret')).not.toBeInTheDocument();
  });

  it("falls back to the untitled label when a protected note has no title yet", () => {
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Secret</p>', 1000, false, false, '', null, null, true)]} />);
    expect(screen.getByText('Ohne Titel')).toBeInTheDocument();
    expect(screen.queryByText('Secret')).not.toBeInTheDocument();
  });

  it("an unprotected note still renders its preview, not the title", () => {
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Visible</p>')]} />);
    expect(screen.getByText('Visible')).toBeInTheDocument();
  });

  it("note context menu offers 'Notiz sperren' and calls onProtectNote(id, true)", () => {
    const onProtectNote = vi.fn();
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Note</p>')]} onProtectNote={onProtectNote} />);
    fireEvent.contextMenu(screen.getByText('Note'));
    fireEvent.click(screen.getByText('Notiz sperren'));
    expect(onProtectNote).toHaveBeenCalledWith('a', true);
  });

  it("a protected note's context menu offers 'Notiz entsperren' and calls onProtectNote(id, false)", () => {
    const onProtectNote = vi.fn();
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Note</p>', 1000, false, false, '', null, null, true, 'Note')]} onProtectNote={onProtectNote} />);
    fireEvent.contextMenu(screen.getByText('Note'));
    fireEvent.click(screen.getByText('Notiz entsperren'));
    expect(onProtectNote).toHaveBeenCalledWith('a', false);
  });

  it("folder context menu offers 'Ordner sperren' and calls onLockFolder(id, true)", () => {
    const onLockFolder = vi.fn();
    const folders = [{ id: 'f1', name: 'Arbeit', parentId: null, position: 1, icon: '', color: '', sort: 'manual', locked: false }];
    render(<NoteList {...defaultProps} folders={folders} onLockFolder={onLockFolder} />);
    fireEvent.contextMenu(screen.getByText('Arbeit'));
    fireEvent.click(screen.getByText('Ordner sperren'));
    expect(onLockFolder).toHaveBeenCalledWith('f1', true);
  });

  it("a locked folder's context menu offers 'Ordner entsperren' and calls onLockFolder(id, false)", () => {
    const onLockFolder = vi.fn();
    const folders = [{ id: 'f1', name: 'Arbeit', parentId: null, position: 1, icon: '', color: '', sort: 'manual', locked: true }];
    render(<NoteList {...defaultProps} folders={folders} onLockFolder={onLockFolder} />);
    fireEvent.contextMenu(screen.getByText('Arbeit'));
    fireEvent.click(screen.getByText('Ordner entsperren'));
    expect(onLockFolder).toHaveBeenCalledWith('f1', false);
  });
});

describe('NoteList — vault lock button', () => {
  it('is hidden when the vault does not exist', () => {
    render(<NoteList {...defaultProps} vaultExists={false} vaultUnlocked={false} />);
    expect(screen.queryByTitle('Jetzt sperren')).not.toBeInTheDocument();
  });

  it('is hidden when the vault exists but is locked', () => {
    render(<NoteList {...defaultProps} vaultExists={true} vaultUnlocked={false} />);
    expect(screen.queryByTitle('Jetzt sperren')).not.toBeInTheDocument();
  });

  it('renders and calls onLockVault when the vault exists and is unlocked', () => {
    const onLockVault = vi.fn();
    render(<NoteList {...defaultProps} vaultExists={true} vaultUnlocked={true} onLockVault={onLockVault} />);
    fireEvent.click(screen.getByTitle('Jetzt sperren'));
    expect(onLockVault).toHaveBeenCalledOnce();
  });
});

describe("NoteList — easter egg", () => {
  it("four quick logo clicks open tic-tac-toe", () => {
    render(<NoteList {...defaultProps} />);
    const logo = screen.getByAltText('Notefix');
    for (let i = 0; i < 4; i++) fireEvent.click(logo);
    expect(screen.getByLabelText('Feld 0')).toBeInTheDocument();
  });

  it("closing the game hides the board again", () => {
    render(<NoteList {...defaultProps} />);
    const logo = screen.getByAltText('Notefix');
    for (let i = 0; i < 4; i++) fireEvent.click(logo);
    expect(screen.getByLabelText('Feld 0')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Schließen'));
    expect(screen.queryByLabelText('Feld 0')).not.toBeInTheDocument();
  });
});

const folder = (o: Partial<Folder> = {}): Folder =>
  ({ id: 'f1', name: 'Arbeit', parentId: null, position: 0, icon: '', color: '', sort: 'manual', locked: false, mcpHidden: false, ...o } as Folder);

describe("NoteList — folder tree: create / rename / sort", () => {
  it("right-clicking the empty list area opens a root menu whose 'Neuer Ordner' calls onCreateFolder(name, null)", async () => {
    const onCreateFolder = vi.fn().mockResolvedValue('f-new');
    const { container } = render(<NoteList {...defaultProps} onCreateFolder={onCreateFolder} />);
    const scrollArea = container.querySelector('.overflow-y-auto')!;
    fireEvent.contextMenu(scrollArea);
    fireEvent.click(screen.getByText('Neuer Ordner'));
    await waitFor(() => expect(onCreateFolder).toHaveBeenCalledWith('Neuer Ordner', null));
  });

  it("header menu's 'Neuer Ordner' also calls onCreateFolder(name, null) when the view is active", async () => {
    const onCreateFolder = vi.fn().mockResolvedValue('f-new');
    render(<NoteList {...defaultProps} onCreateFolder={onCreateFolder} />);
    fireEvent.click(screen.getByTitle('Mehr'));
    fireEvent.click(screen.getByText('Neuer Ordner'));
    await waitFor(() => expect(onCreateFolder).toHaveBeenCalledWith('Neuer Ordner', null));
  });

  it("header menu offers 'Aktive Notizen' to leave the archive view", () => {
    render(<NoteList {...defaultProps} />);
    fireEvent.click(screen.getByTitle('Mehr'));
    fireEvent.click(screen.getByText('Archiv anzeigen'));
    expect(screen.getByText('Archiv')).toBeInTheDocument();
    fireEvent.click(screen.getByTitle('Mehr'));
    fireEvent.click(screen.getByText('Aktive Notizen'));
    expect(screen.queryByText('Archiv')).not.toBeInTheDocument();
  });

  it("folder menu 'Neue Notiz hier' creates a note, moves it into the folder and expands it", async () => {
    const onCreate = vi.fn().mockResolvedValue('new-note');
    const onMoveNote = vi.fn();
    render(<NoteList {...defaultProps} folders={[folder()]} onCreate={onCreate} onMoveNote={onMoveNote} />);
    fireEvent.contextMenu(screen.getByText('Arbeit'));
    fireEvent.click(screen.getByText('Neue Notiz hier'));
    await waitFor(() => expect(onMoveNote).toHaveBeenCalledWith('new-note', 'f1'));
  });

  it("folder menu 'Neuer Unterordner' expands the parent and calls onCreateFolder with the parent id", async () => {
    const onCreateFolder = vi.fn().mockResolvedValue('sub1');
    render(<NoteList {...defaultProps} folders={[folder()]} onCreateFolder={onCreateFolder} />);
    fireEvent.contextMenu(screen.getByText('Arbeit'));
    fireEvent.click(screen.getByText('Neuer Unterordner'));
    await waitFor(() => expect(onCreateFolder).toHaveBeenCalledWith('Neuer Ordner', 'f1'));
  });

  it("folder rename: 'Umbenennen' shows an inline input; Enter blurs and calls onRenameFolder with the trimmed value", () => {
    const onRenameFolder = vi.fn();
    render(<NoteList {...defaultProps} folders={[folder()]} onRenameFolder={onRenameFolder} />);
    fireEvent.contextMenu(screen.getByText('Arbeit'));
    fireEvent.click(screen.getByText('Umbenennen'));
    const input = screen.getByDisplayValue('Arbeit') as HTMLInputElement;
    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: '  Neu  ' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(onRenameFolder).toHaveBeenCalledWith('f1', 'Neu');
  });

  it("folder rename: Escape cancels editing without calling onRenameFolder", () => {
    const onRenameFolder = vi.fn();
    render(<NoteList {...defaultProps} folders={[folder()]} onRenameFolder={onRenameFolder} />);
    fireEvent.contextMenu(screen.getByText('Arbeit'));
    fireEvent.click(screen.getByText('Umbenennen'));
    const input = screen.getByDisplayValue('Arbeit');
    fireEvent.keyDown(input, { key: 'Escape' });
    expect(screen.queryByDisplayValue('Arbeit')).not.toBeInTheDocument();
    expect(screen.getByText('Arbeit')).toBeInTheDocument();
    expect(onRenameFolder).not.toHaveBeenCalled();
  });

  it("folder menu 'Sortierung' submenu picks a sort option and calls onSetFolderSort", () => {
    const onSetFolderSort = vi.fn();
    render(<NoteList {...defaultProps} folders={[folder()]} onSetFolderSort={onSetFolderSort} />);
    fireEvent.contextMenu(screen.getByText('Arbeit'));
    fireEvent.click(screen.getByText('Sortierung'));
    fireEvent.click(screen.getByText('Titel A–Z'));
    expect(onSetFolderSort).toHaveBeenCalledWith('f1', 'titleAsc');
  });

  it("folder menu 'Löschen' calls onDeleteFolder with the folder", () => {
    const onDeleteFolder = vi.fn();
    const f = folder();
    render(<NoteList {...defaultProps} folders={[f]} onDeleteFolder={onDeleteFolder} />);
    fireEvent.contextMenu(screen.getByText('Arbeit'));
    fireEvent.click(screen.getByText('Löschen'));
    expect(onDeleteFolder).toHaveBeenCalledWith(f);
  });

  it("folder menu offers 'Vor KI verbergen' and calls onSetFolderMcpHidden(id, true)", () => {
    const onSetFolderMcpHidden = vi.fn();
    render(<NoteList {...defaultProps} folders={[folder({ mcpHidden: false })]} onSetFolderMcpHidden={onSetFolderMcpHidden} />);
    fireEvent.contextMenu(screen.getByText('Arbeit'));
    fireEvent.click(screen.getByText('Vor KI verbergen'));
    expect(onSetFolderMcpHidden).toHaveBeenCalledWith('f1', true);
  });

  it("an already MCP-hidden folder's menu offers 'KI zeigen' and calls onSetFolderMcpHidden(id, false)", () => {
    const onSetFolderMcpHidden = vi.fn();
    render(<NoteList {...defaultProps} folders={[folder({ mcpHidden: true })]} onSetFolderMcpHidden={onSetFolderMcpHidden} />);
    fireEvent.contextMenu(screen.getByText('Arbeit'));
    fireEvent.click(screen.getByText('KI zeigen'));
    expect(onSetFolderMcpHidden).toHaveBeenCalledWith('f1', false);
  });

  it("folder customizer: picking Standard calls onSetFolderIcon; picking a swatch calls onSetFolderColor", () => {
    const onSetFolderIcon = vi.fn();
    const onSetFolderColor = vi.fn();
    render(<NoteList {...defaultProps} folders={[folder()]} onSetFolderIcon={onSetFolderIcon} onSetFolderColor={onSetFolderColor} />);
    fireEvent.contextMenu(screen.getByText('Arbeit'));
    fireEvent.click(screen.getByText('Anpassen…'));
    fireEvent.click(screen.getByText('Standard'));
    expect(onSetFolderIcon).toHaveBeenCalledWith('f1', '');
    fireEvent.click(screen.getByLabelText('Farbe #22c55e'));
    expect(onSetFolderColor).toHaveBeenCalledWith('f1', '#22c55e');
  });

  it("nested subfolders render recursively and sibling folders sort by position then name", () => {
    const folders = [
      folder({ id: 'f2', name: 'Zebra', position: 1 }),
      folder({ id: 'f1', name: 'Alpha', position: 1 }),
      folder({ id: 'f3', name: 'Child', parentId: 'f1', position: 0 }),
    ];
    render(<NoteList {...defaultProps} folders={folders} />);
    const rootNames = screen.getAllByText(/^Alpha$|^Zebra$/).map(e => e.textContent);
    expect(rootNames).toEqual(['Alpha', 'Zebra']); // tie on position -> alphabetical
    expect(screen.queryByText('Child')).not.toBeInTheDocument(); // collapsed
    fireEvent.click(screen.getByText('Alpha'));
    expect(screen.getByText('Child')).toBeInTheDocument();
  });

  it("folderColorStyle 'row' tints the row background with the folder color", () => {
    const { container } = render(<NoteList {...defaultProps} folders={[folder({ color: '#ef4444' })]} folderColorStyle="row" />);
    expect(container.querySelector('[style*="rgba(239, 68, 68"]')).toBeTruthy();
  });

  it("folderColorStyle 'bar' draws a colored left border", () => {
    const { container } = render(<NoteList {...defaultProps} folders={[folder({ color: '#ef4444' })]} folderColorStyle="bar" />);
    expect(container.querySelector('[style*="border-left: 3px solid rgb(239, 68, 68)"]')).toBeTruthy();
  });
});

describe("NoteList — note context menu actions", () => {
  it("'Archivieren' calls onArchive(id, true)", () => {
    const onArchive = vi.fn();
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Note</p>')]} onArchive={onArchive} />);
    fireEvent.contextMenu(screen.getByText('Note'));
    fireEvent.click(screen.getByText('Archivieren'));
    expect(onArchive).toHaveBeenCalledWith('a', true);
  });

  it("'Exportieren' calls onExportNote with the note", () => {
    const onExportNote = vi.fn();
    const n = note('a', '<p>Note</p>');
    render(<NoteList {...defaultProps} notes={[n]} onExportNote={onExportNote} />);
    fireEvent.contextMenu(screen.getByText('Note'));
    fireEvent.click(screen.getByText('Exportieren'));
    expect(onExportNote).toHaveBeenCalledWith(n);
  });

  it("'Notiz darüber' creates a sibling note above via onCreate + onReorderNotes", async () => {
    const onCreate = vi.fn().mockResolvedValue('new-id');
    const onReorderNotes = vi.fn();
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Note</p>')]} onCreate={onCreate} onReorderNotes={onReorderNotes} />);
    fireEvent.contextMenu(screen.getByText('Note'));
    fireEvent.click(screen.getByText('Notiz darüber'));
    await waitFor(() => expect(onReorderNotes).toHaveBeenCalledWith(null, ['new-id', 'a']));
  });

  it("'Notiz darunter' creates a sibling note below via onCreate + onReorderNotes", async () => {
    const onCreate = vi.fn().mockResolvedValue('new-id');
    const onReorderNotes = vi.fn();
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Note</p>')]} onCreate={onCreate} onReorderNotes={onReorderNotes} />);
    fireEvent.contextMenu(screen.getByText('Note'));
    fireEvent.click(screen.getByText('Notiz darunter'));
    await waitFor(() => expect(onReorderNotes).toHaveBeenCalledWith(null, ['a', 'new-id']));
  });

  it("'Vor KI verbergen' calls onSetNoteMcpHidden(id, true)", () => {
    const onSetNoteMcpHidden = vi.fn();
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Note</p>')]} onSetNoteMcpHidden={onSetNoteMcpHidden} />);
    fireEvent.contextMenu(screen.getByText('Note'));
    fireEvent.click(screen.getByText('Vor KI verbergen'));
    expect(onSetNoteMcpHidden).toHaveBeenCalledWith('a', true);
  });

  it("an already MCP-hidden note offers 'KI zeigen' and calls onSetNoteMcpHidden(id, false)", () => {
    const onSetNoteMcpHidden = vi.fn();
    const n = { ...note('a', '<p>Note</p>'), mcpHidden: true };
    render(<NoteList {...defaultProps} notes={[n]} onSetNoteMcpHidden={onSetNoteMcpHidden} />);
    fireEvent.contextMenu(screen.getByText('Note'));
    fireEvent.click(screen.getByText('KI zeigen'));
    expect(onSetNoteMcpHidden).toHaveBeenCalledWith('a', false);
  });

  it("move-to submenu lists nested folders and calls onMoveNote for a nested folder", () => {
    const onMoveNote = vi.fn();
    const folders = [folder({ id: 'f1', name: 'Parent' }), folder({ id: 'f2', name: 'Child', parentId: 'f1' })];
    render(<NoteList {...defaultProps} folders={folders} notes={[note('a', '<p>Root note</p>')]} onMoveNote={onMoveNote} />);
    fireEvent.contextMenu(screen.getByText('Root note'));
    fireEvent.click(screen.getByText('Verschieben nach'));
    fireEvent.click(screen.getByText('Child'));
    expect(onMoveNote).toHaveBeenCalledWith('a', 'f2');
  });
});

describe("NoteList — pinnedScope 'global'", () => {
  it("shows a pinned section above the tree, including pinned notes nested in a collapsed folder", () => {
    const notes = [
      note('a', '<p>A</p>', 1, true, false, '', null, null),
      note('b', '<p>B</p>', 2, true, false, '', null, 'f1'),
      note('c', '<p>C</p>', 3, false, false, '', null, null),
    ];
    render(<NoteList {...defaultProps} notes={notes} folders={[folder()]} pinnedScope="global" />);
    expect(screen.getByText('Angepinnt')).toBeInTheDocument();
    expect(screen.getByText('A')).toBeInTheDocument();
    expect(screen.getByText('B')).toBeInTheDocument(); // pinned, shown even though folder f1 is collapsed
    expect(screen.getByText('C')).toBeInTheDocument();
  });
});

describe("NoteList — trash view", () => {
  it("lists trashed notes and 'Löschen' opens a purge confirm; confirming calls onPurge", () => {
    const onPurge = vi.fn();
    render(<NoteList {...defaultProps} trashed={[note('t', '<p>Weg</p>')]} onPurge={onPurge} />);
    fireEvent.click(screen.getByTitle('Mehr'));
    fireEvent.click(screen.getByText('Papierkorb'));
    fireEvent.click(screen.getByTitle('Endgültig löschen'));
    fireEvent.click(screen.getByRole('button', { name: 'Endgültig löschen' }));
    expect(onPurge).toHaveBeenCalledWith('t');
  });

  it("'Papierkorb leeren' opens a confirm; 'Leeren' calls onEmptyTrash", () => {
    const onEmptyTrash = vi.fn();
    render(<NoteList {...defaultProps} trashed={[note('t', '<p>Weg</p>')]} onEmptyTrash={onEmptyTrash} />);
    fireEvent.click(screen.getByTitle('Mehr'));
    fireEvent.click(screen.getByText('Papierkorb'));
    fireEvent.click(screen.getByText('Papierkorb leeren'));
    fireEvent.click(screen.getByText('Leeren'));
    expect(onEmptyTrash).toHaveBeenCalledOnce();
  });

  it("shows a protected trashed note's title with a lock icon instead of its preview", () => {
    render(<NoteList {...defaultProps} trashed={[note('t', '<p>Secret</p>', 1, false, false, '', null, null, true, 'Secret Title')]} />);
    fireEvent.click(screen.getByTitle('Mehr'));
    fireEvent.click(screen.getByText('Papierkorb'));
    expect(screen.getByText('Secret Title')).toBeInTheDocument();
    expect(screen.queryByText('Secret')).not.toBeInTheDocument();
  });
});

describe("NoteList — delete/purge/empty confirm cancel", () => {
  it("cancelling the delete confirm does not call onDelete", () => {
    const onDelete = vi.fn();
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Note</p>')]} onDelete={onDelete} />);
    fireEvent.click(screen.getByTitle('Notiz löschen'));
    fireEvent.click(screen.getByText('Abbrechen'));
    expect(onDelete).not.toHaveBeenCalled();
    expect(screen.queryByText('In Papierkorb')).not.toBeInTheDocument();
  });

  it("cancelling the purge confirm does not call onPurge", () => {
    const onPurge = vi.fn();
    render(<NoteList {...defaultProps} trashed={[note('t', '<p>Weg</p>')]} onPurge={onPurge} />);
    fireEvent.click(screen.getByTitle('Mehr'));
    fireEvent.click(screen.getByText('Papierkorb'));
    fireEvent.click(screen.getByTitle('Endgültig löschen'));
    fireEvent.click(screen.getByText('Abbrechen'));
    expect(onPurge).not.toHaveBeenCalled();
  });

  it("cancelling 'Papierkorb leeren' does not call onEmptyTrash", () => {
    const onEmptyTrash = vi.fn();
    render(<NoteList {...defaultProps} trashed={[note('t', '<p>Weg</p>')]} onEmptyTrash={onEmptyTrash} />);
    fireEvent.click(screen.getByTitle('Mehr'));
    fireEvent.click(screen.getByText('Papierkorb'));
    fireEvent.click(screen.getByText('Papierkorb leeren'));
    fireEvent.click(screen.getByText('Abbrechen'));
    expect(onEmptyTrash).not.toHaveBeenCalled();
  });

  it("when trashEnabled is false, the delete confirm uses permanent-delete wording", () => {
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Note</p>')]} trashEnabled={false} />);
    fireEvent.click(screen.getByTitle('Notiz löschen'));
    expect(screen.getByText('Diese Notiz endgültig löschen?')).toBeInTheDocument();
  });
});

describe("NoteList — drag and drop", () => {
  it("dragging a note before a sibling reorders notes via onReorderNotes", () => {
    const onReorderNotes = vi.fn();
    const notes = [note('a', '<p>A</p>'), note('b', '<p>B</p>')];
    render(<NoteList {...defaultProps} notes={notes} onReorderNotes={onReorderNotes} />);
    act(() => dndRef.current.onDragStart({ active: { id: 'note:a' } }));
    act(() => dndRef.current.onDragOver({ over: { id: 'note:b:before' } }));
    act(() => dndRef.current.onDragEnd({ active: { id: 'note:a' }, over: { id: 'note:b:before' } }));
    expect(onReorderNotes).toHaveBeenCalledWith(null, ['a', 'b']);
  });

  it("dropping with no target clears the hint without reordering; onDragCancel resets drag state", () => {
    const onReorderNotes = vi.fn();
    render(<NoteList {...defaultProps} notes={[note('a', '<p>A</p>')]} onReorderNotes={onReorderNotes} />);
    act(() => dndRef.current.onDragStart({ active: { id: 'note:a' } }));
    act(() => dndRef.current.onDragOver({ over: null }));
    act(() => dndRef.current.onDragEnd({ active: { id: 'note:a' }, over: null }));
    expect(onReorderNotes).not.toHaveBeenCalled();
    act(() => dndRef.current.onDragCancel());
    expect(document.querySelector('.max-w-56')).toBeFalsy();
  });

  it("dragging a note onto the root drop zone moves it to root via onReorderNotes", () => {
    const onReorderNotes = vi.fn();
    const notes = [note('a', '<p>A</p>', 1, false, false, '', null, 'f1')];
    render(<NoteList {...defaultProps} folders={[folder()]} notes={notes} onReorderNotes={onReorderNotes} />);
    fireEvent.click(screen.getByText('Arbeit')); // expand so the note is in the DOM
    act(() => dndRef.current.onDragStart({ active: { id: 'note:a' } }));
    act(() => dndRef.current.onDragEnd({ active: { id: 'note:a' }, over: { id: 'root:into' } }));
    expect(onReorderNotes).toHaveBeenCalledWith(null, ['a']);
  });

  it("dragging a folder into another folder reorders via onReorderFolders", () => {
    const onReorderFolders = vi.fn();
    const folders = [folder({ id: 'f1', name: 'A' }), folder({ id: 'f2', name: 'B' })];
    render(<NoteList {...defaultProps} folders={folders} onReorderFolders={onReorderFolders} />);
    act(() => dndRef.current.onDragStart({ active: { id: 'folder:f1' } }));
    act(() => dndRef.current.onDragEnd({ active: { id: 'folder:f1' }, over: { id: 'folder:f2:into' } }));
    expect(onReorderFolders).toHaveBeenCalledWith('f2', ['f1']);
  });

  it("drag overlay shows the dragged note's preview text", () => {
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Preview text</p>')]} />);
    act(() => dndRef.current.onDragStart({ active: { id: 'note:a' } }));
    const overlay = document.querySelector('.max-w-56');
    expect(overlay?.textContent).toBe('Preview text');
  });

  it("drag overlay shows a protected note's title with a lock icon, not its preview", () => {
    render(<NoteList {...defaultProps} notes={[note('a', '<p>Secret</p>', 1, false, false, '', null, null, true, 'Secret Title')]} />);
    act(() => dndRef.current.onDragStart({ active: { id: 'note:a' } }));
    const overlay = document.querySelector('.max-w-56');
    expect(overlay?.textContent).toBe('Secret Title');
    expect(overlay?.querySelector('[data-icon="lock"]')).toBeTruthy();
  });

  it("drag overlay shows the folder name while dragging a folder", () => {
    render(<NoteList {...defaultProps} folders={[folder()]} />);
    act(() => dndRef.current.onDragStart({ active: { id: 'folder:f1' } }));
    const overlay = document.querySelector('.max-w-56');
    expect(overlay?.textContent).toBe('Arbeit');
  });
});

describe("NoteList — mobile layout", () => {
  it("renders a full-width sidebar on mobile", () => {
    const { container } = render(<NoteList {...defaultProps} mobile />);
    expect(container.querySelector('aside')).toHaveClass('w-full');
    expect(container.querySelector('aside')).not.toHaveClass('w-60');
  });
});
