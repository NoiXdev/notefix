import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import NoteContextMenu, { DeleteNoteDialog } from './NoteContextMenu';
import type { NoteMeta, Folder } from '../types';

const platformState = vi.hoisted(() => ({ isMobilePlatform: false }));
vi.mock('../platform', () => ({
  get isMobilePlatform() { return platformState.isMobilePlatform; },
}));

const note = (over: Partial<NoteMeta> = {}): NoteMeta => ({
  id: 'n', updatedAt: 1, pinned: false, archived: false, color: '', dueAt: null, folderId: null,
  position: 0, deletedAt: null, preview: 'N', tasksDone: 0, tasksTotal: 0, protected: false, title: '', mcpHidden: false,
  ...over,
} as NoteMeta);

const folder = (id: string, name: string, parentId: string | null = null, position = 0): Folder =>
  ({ id, name, parentId, position } as Folder);

const base = () => ({
  x: 10, y: 10, folders: [] as Folder[], onClose: vi.fn(), onRequestDelete: vi.fn(), onExportNote: vi.fn(),
});

describe('NoteContextMenu', () => {
  beforeEach(() => { platformState.isMobilePlatform = false; });

  it('offers only delete and export without optional handlers', () => {
    render(<NoteContextMenu {...base()} note={note()} />);
    expect(screen.getAllByRole('button').map(b => b.textContent)).toEqual(['Löschen', 'Exportieren']);
  });

  it('routes every action to its handler', () => {
    const p = { ...base(), onCreateBeside: vi.fn(), onTogglePin: vi.fn(), onArchive: vi.fn(), onPrintNote: vi.fn(), onProtectNote: vi.fn(), onSetNoteMcpHidden: vi.fn() };
    const n = note();
    const { rerender } = render(<NoteContextMenu {...p} note={n} />);
    fireEvent.click(screen.getByText('Notiz darüber'));
    fireEvent.click(screen.getByText('Notiz darunter'));
    fireEvent.click(screen.getByText('Anpinnen'));
    fireEvent.click(screen.getByText('Archivieren'));
    fireEvent.click(screen.getByText('Löschen'));
    fireEvent.click(screen.getByText('Exportieren'));
    fireEvent.click(screen.getByText('Drucken'));
    fireEvent.click(screen.getByText('Notiz sperren'));
    fireEvent.click(screen.getByText('Vor KI verbergen'));
    expect(p.onCreateBeside).toHaveBeenCalledWith(n, 'before');
    expect(p.onCreateBeside).toHaveBeenCalledWith(n, 'after');
    expect(p.onTogglePin).toHaveBeenCalledWith('n', true);
    expect(p.onArchive).toHaveBeenCalledWith('n', true);
    expect(p.onRequestDelete).toHaveBeenCalledWith('n');
    expect(p.onExportNote).toHaveBeenCalledWith(n);
    expect(p.onPrintNote).toHaveBeenCalledWith(n);
    expect(p.onProtectNote).toHaveBeenCalledWith('n', true);
    expect(p.onSetNoteMcpHidden).toHaveBeenCalledWith('n', true);

    const flipped = note({ pinned: true, archived: true, protected: true, mcpHidden: true });
    rerender(<NoteContextMenu {...p} note={flipped} vaultUnlocked />);
    fireEvent.click(screen.getByText('Lösen'));
    fireEvent.click(screen.getByText('Wiederherstellen'));
    fireEvent.click(screen.getByText('Notiz entsperren'));
    fireEvent.click(screen.getByText('KI zeigen'));
    expect(p.onTogglePin).toHaveBeenLastCalledWith('n', false);
    expect(p.onArchive).toHaveBeenLastCalledWith('n', false);
    expect(p.onProtectNote).toHaveBeenLastCalledWith('n', false);
    expect(p.onSetNoteMcpHidden).toHaveBeenLastCalledWith('n', false);
  });

  it('hides print for a locked protected note and on mobile platforms', () => {
    const { rerender } = render(<NoteContextMenu {...base()} onPrintNote={vi.fn()} note={note({ protected: true })} vaultUnlocked={false} />);
    expect(screen.queryByText('Drucken')).not.toBeInTheDocument();
    platformState.isMobilePlatform = true;
    rerender(<NoteContextMenu {...base()} onPrintNote={vi.fn()} note={note()} />);
    expect(screen.queryByText('Drucken')).not.toBeInTheDocument();
  });

  it('lists root and nested folders, in order, in the move submenu', () => {
    const onMoveNote = vi.fn();
    const folders = [folder('b', 'Beta', null, 1), folder('a', 'Alpha', null, 0), folder('c', 'Child', 'a')];
    render(<NoteContextMenu {...base()} folders={folders} onMoveNote={onMoveNote} note={note()} />);
    fireEvent.mouseEnter(screen.getByText('Verschieben nach').closest('div')!);
    const labels = screen.getAllByRole('button').map(b => b.textContent?.trim());
    expect(labels).toEqual(expect.arrayContaining(['— Root —', 'Alpha', 'Child', 'Beta']));
    expect(labels.indexOf('Child')).toBe(labels.indexOf('Alpha') + 1);
    fireEvent.click(screen.getByText('— Root —'));
    expect(onMoveNote).toHaveBeenCalledWith('n', null);
    fireEvent.mouseEnter(screen.getByText('Verschieben nach').closest('div')!);
    fireEvent.click(screen.getByText(/Child/));
    expect(onMoveNote).toHaveBeenCalledWith('n', 'c');
  });

  it('colour swatches set the note colour', () => {
    const onSetColor = vi.fn();
    render(<NoteContextMenu {...base()} onSetColor={onSetColor} note={note()} />);
    fireEvent.click(screen.getByLabelText('Keine Farbe'));
    expect(onSetColor).toHaveBeenCalledWith('n', '');
  });
});

describe('DeleteNoteDialog', () => {
  it('words the confirmation for the trash or a permanent delete', () => {
    const onConfirm = vi.fn();
    const { rerender } = render(<DeleteNoteDialog trashEnabled onConfirm={onConfirm} onCancel={vi.fn()} />);
    fireEvent.click(screen.getByText('In Papierkorb'));
    expect(onConfirm).toHaveBeenCalledOnce();
    rerender(<DeleteNoteDialog trashEnabled={false} onConfirm={onConfirm} onCancel={vi.fn()} />);
    expect(screen.getAllByText('Endgültig löschen').length).toBeGreaterThan(0);
  });
});
