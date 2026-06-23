import { useState, useRef } from 'react';
import type { Note, Folder } from '../types';
import { computeDrop, type DragKind, type DropMode } from '../dnd';
import type { PinnedScope } from '../hooks/useSettings';
import ContextMenu, { type ContextMenuItem } from './ContextMenu';
import { NOTE_COLORS, DEFAULT_MARKER } from '../colors';
import { exportSelected } from '../export';
import { formatDate, type DateFormat } from '../dates';

interface Props {
  notes: Note[];
  folders: Folder[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreate: () => void;
  onDelete: (id: string) => void;
  onOpenSettings: () => void;
  onTogglePin?: (id: string, pinned: boolean) => void;
  onArchive?: (id: string, archived: boolean) => void;
  onSetColor?: (id: string, color: string) => void;
  onMoveNote?: (id: string, folderId: string | null) => void;
  onCreateFolder?: (name: string, parentId: string | null) => Promise<string>;
  onRenameFolder?: (id: string, name: string) => void;
  onDeleteFolder?: (folder: Folder) => void;
  onReorderNotes?: (folderId: string | null, ids: string[]) => void;
  onReorderFolders?: (parentId: string | null, ids: string[]) => void;
  dateFormat?: DateFormat;
  pinnedScope?: PinnedScope;
}

export function getPreview(html: string): string {
  const el = document.createElement('div');
  el.innerHTML = html;
  const first = el.firstElementChild;
  const text = first?.textContent?.trim() ?? el.textContent?.trim() ?? '';
  return text.slice(0, 60) || 'New note';
}

const sortNotes = (a: Note, b: Note) => Number(b.pinned) - Number(a.pinned) || b.updatedAt - a.updatedAt;

function PinIcon({ color }: { color: string }) {
  return (
    <svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor" className="shrink-0 mt-1" style={{ color }} aria-hidden>
      <path d="M14 4v5l2 3v2h-5v5l-1 1-1-1v-5H4v-2l2-3V4a1 1 0 0 1-1-1h10a1 1 0 0 1-1 1z" />
    </svg>
  );
}

export default function NoteList(props: Props) {
  const {
    notes, folders, selectedId, onSelect, onCreate, onDelete, onOpenSettings,
    onTogglePin, onArchive, onSetColor, onMoveNote, onCreateFolder, onRenameFolder, onDeleteFolder,
    onReorderNotes, onReorderFolders,
    dateFormat = 'auto', pinnedScope = 'perFolder',
  } = props;

  const [menu, setMenu] = useState<{ x: number; y: number; note: Note } | null>(null);
  const [folderMenu, setFolderMenu] = useState<{ x: number; y: number; folder: Folder } | null>(null);
  const [showArchived, setShowArchived] = useState(false);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [editingFolder, setEditingFolder] = useState<string | null>(null);
  const [rootMenu, setRootMenu] = useState<{ x: number; y: number } | null>(null);

  const toggle = (id: string) => setExpanded(prev => { const n = new Set(prev); n.has(id) ? n.delete(id) : n.add(id); return n; });

  // Create a folder, then immediately put it into inline-rename. createFolder
  // returns the new id once the list has reloaded, so editing targets a real row.
  const createAndEdit = (parentId: string | null) => {
    if (!onCreateFolder) return;
    if (parentId) setExpanded(prev => { const n = new Set(prev); n.add(parentId); return n; });
    void onCreateFolder('Neuer Ordner', parentId).then(id => setEditingFolder(id));
  };

  const dragRef = useRef<{ kind: DragKind; id: string } | null>(null);
  const [dropHint, setDropHint] = useState<{ id: string; mode: DropMode } | null>(null);

  const finishDrop = (targetKind: DragKind | 'root', targetId: string | null, mode: DropMode) => {
    const d = dragRef.current;
    dragRef.current = null; setDropHint(null);
    if (!d) return;
    const res = computeDrop({ draggedKind: d.kind, draggedId: d.id, targetKind, targetId, mode, notes, folders });
    if (!res) return;
    if (res.kind === 'note') onReorderNotes?.(res.parentId, res.orderedIds);
    else onReorderFolders?.(res.parentId, res.orderedIds);
  };

  const noteModeAt = (e: React.DragEvent): DropMode => {
    const r = e.currentTarget.getBoundingClientRect();
    return (e.clientY - r.top) < r.height / 2 ? 'before' : 'after';
  };
  const folderModeAt = (e: React.DragEvent): DropMode => {
    const r = e.currentTarget.getBoundingClientRect();
    const y = e.clientY - r.top;
    return y < r.height / 3 ? 'before' : y > (r.height * 2) / 3 ? 'into' : 'into';
  };

  // Move-to submenu: all folders indented by depth + root.
  const moveSubmenu = (note: Note): ContextMenuItem[] => {
    const byParent = (pid: string | null): Folder[] => folders.filter(f => (f.parentId ?? null) === pid).sort((a, b) => a.position - b.position);
    const items: ContextMenuItem[] = [{ label: '— Root —', onClick: () => onMoveNote?.(note.id, null) }];
    const walk = (pid: string | null, depth: number) => {
      for (const f of byParent(pid)) {
        items.push({ label: `${'  '.repeat(depth)}${f.name}`, onClick: () => onMoveNote?.(note.id, f.id) });
        walk(f.id, depth + 1);
      }
    };
    walk(null, 0);
    return items;
  };

  const renderRow = (note: Note, depth: number) => {
    const marker = note.color || DEFAULT_MARKER;
    return (
      <button
        key={note.id}
        onClick={() => onSelect(note.id)}
        onContextMenu={e => { e.preventDefault(); setMenu({ x: e.clientX, y: e.clientY, note }); }}
        className={`w-full text-left py-3 border-b border-gray-900 group relative transition-colors ${selectedId === note.id ? 'bg-gray-800' : 'hover:bg-gray-900'}`}
        draggable
        onDragStart={e => { dragRef.current = { kind: 'note', id: note.id }; e.dataTransfer.effectAllowed = 'move'; }}
        onDragEnd={() => { dragRef.current = null; setDropHint(null); }}
        onDragOver={e => { e.preventDefault(); setDropHint({ id: note.id, mode: noteModeAt(e) }); }}
        onDrop={e => { e.preventDefault(); e.stopPropagation(); finishDrop('note', note.id, noteModeAt(e)); }}
        style={{ paddingLeft: 16 + depth * 14, paddingRight: 16, boxShadow: dropHint?.id === note.id ? (dropHint.mode === 'before' ? 'inset 0 2px 0 #fde047' : 'inset 0 -2px 0 #fde047') : undefined }}
      >
        <div className="flex items-start gap-2">
          {note.pinned ? <PinIcon color={marker} /> : <div className="w-2 h-2 rounded-sm shrink-0 mt-1.5" style={{ background: marker }} />}
          <div className="min-w-0 flex-1">
            <div className="text-gray-100 text-sm font-medium truncate pr-5 leading-snug">{getPreview(note.content)}</div>
            <div className="text-gray-500 text-xs mt-0.5 flex items-center gap-2">
              <span>{formatDate(note.updatedAt, dateFormat)}</span>
              {note.dueAt != null && (
                <span className="inline-flex items-center gap-1 px-1.5 rounded" style={note.dueAt < Date.now() ? { background: '#fee2e2', color: '#b91c1c' } : { background: '#1f2937', color: '#9ca3af' }} title="Fällig">
                  <svg width="9" height="9" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="3" y="4" width="18" height="18" rx="2" /><line x1="3" y1="10" x2="21" y2="10" /></svg>
                  {formatDate(note.dueAt, dateFormat)}
                </span>
              )}
            </div>
          </div>
        </div>
        <span onClick={e => { e.stopPropagation(); onDelete(note.id); }} className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-600 hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity" title="Delete note">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><polyline points="3 6 5 6 21 6" /><path d="M19 6l-1 14H6L5 6" /></svg>
        </span>
      </button>
    );
  };

  // active (non-archived) notes for the tree; archived rendered flat.
  const activeNotes = notes.filter(n => !n.archived);
  const archivedNotes = notes.filter(n => n.archived);

  const treeNotesIn = (fid: string | null) =>
    activeNotes.filter(n => (n.folderId ?? null) === fid && (pinnedScope === 'global' ? !n.pinned : true)).sort(sortNotes);

  const childFolders = (pid: string | null) => folders.filter(f => (f.parentId ?? null) === pid).sort((a, b) => a.position - b.position || a.name.localeCompare(b.name));

  const renderFolder = (folder: Folder, depth: number) => {
    const open = expanded.has(folder.id);
    const count = activeNotes.filter(n => (n.folderId ?? null) === folder.id).length;
    return (
      <div key={folder.id}>
        {editingFolder === folder.id ? (
          <input
            autoFocus
            defaultValue={folder.name}
            onBlur={e => { onRenameFolder?.(folder.id, e.target.value.trim() || folder.name); setEditingFolder(null); }}
            onKeyDown={e => { if (e.key === 'Enter') (e.target as HTMLInputElement).blur(); if (e.key === 'Escape') setEditingFolder(null); }}
            className="w-full bg-gray-800 text-gray-100 text-sm px-2 py-2 outline-none"
            style={{ marginLeft: 8 + depth * 14 }}
          />
        ) : (
          <div
            onClick={() => toggle(folder.id)}
            onContextMenu={e => { e.preventDefault(); setFolderMenu({ x: e.clientX, y: e.clientY, folder }); }}
            draggable
            onDragStart={e => { e.stopPropagation(); dragRef.current = { kind: 'folder', id: folder.id }; e.dataTransfer.effectAllowed = 'move'; }}
            onDragEnd={() => { dragRef.current = null; setDropHint(null); }}
            onDragOver={e => { e.preventDefault(); e.stopPropagation(); setDropHint({ id: folder.id, mode: folderModeAt(e) }); }}
            onDrop={e => { e.preventDefault(); e.stopPropagation(); finishDrop('folder', folder.id, folderModeAt(e)); }}
            className={`flex items-center gap-1 py-2 text-gray-300 hover:bg-gray-900 cursor-pointer select-none ${dropHint?.id === folder.id && dropHint.mode === 'into' ? 'bg-gray-700' : ''}`}
            style={{ paddingLeft: 8 + depth * 14, paddingRight: 12 }}
          >
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" style={{ transform: open ? 'rotate(90deg)' : 'none' }}><polyline points="9 6 15 12 9 18" /></svg>
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" /></svg>
            <span className="text-sm font-medium truncate flex-1">{folder.name}</span>
            <span className="text-gray-600 text-xs">{count || ''}</span>
          </div>
        )}
        {open && (
          <>
            {childFolders(folder.id).map(f => renderFolder(f, depth + 1))}
            {treeNotesIn(folder.id).map(n => renderRow(n, depth + 1))}
          </>
        )}
      </div>
    );
  };

  const globalPinned = pinnedScope === 'global' ? activeNotes.filter(n => n.pinned).sort(sortNotes) : [];

  return (
    <aside className="w-60 shrink-0 bg-gray-950 flex flex-col h-full select-none">
      <div className="px-4 py-3 flex items-center justify-between border-b border-gray-800">
        <span className="text-gray-400 text-xs font-semibold uppercase tracking-widest">{showArchived ? 'Archiv' : 'Notes'}</span>
        <div className="flex items-center gap-1">
          {!showArchived && onCreateFolder && (
            <button onClick={() => createAndEdit(null)} className="w-6 h-6 flex items-center justify-center text-gray-400 hover:text-white hover:bg-gray-700 rounded" title="Neuer Ordner">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" /><line x1="12" y1="11" x2="12" y2="17" /><line x1="9" y1="14" x2="15" y2="14" /></svg>
            </button>
          )}
          <button onClick={() => setShowArchived(v => !v)} className={`w-6 h-6 flex items-center justify-center rounded ${showArchived ? 'text-white bg-gray-700' : 'text-gray-400 hover:text-white hover:bg-gray-700'}`} title={showArchived ? 'Aktive Notizen' : 'Archiv anzeigen'}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="3" y="4" width="18" height="4" rx="1" /><path d="M5 8v11a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8" /><line x1="10" y1="12" x2="14" y2="12" /></svg>
          </button>
          {!showArchived && (
            <button onClick={onCreate} className="w-6 h-6 flex items-center justify-center text-gray-400 hover:text-white hover:bg-gray-700 rounded" title="New note">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>
            </button>
          )}
          <button onClick={onOpenSettings} className="w-6 h-6 flex items-center justify-center text-gray-400 hover:text-white hover:bg-gray-700 rounded" title="Settings">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" /></svg>
          </button>
        </div>
      </div>

      <div
        className="flex-1 overflow-y-auto"
        onContextMenu={e => { if (e.target === e.currentTarget && onCreateFolder && !showArchived) { e.preventDefault(); setRootMenu({ x: e.clientX, y: e.clientY }); } }}
        onDragOver={e => { if (e.target === e.currentTarget) e.preventDefault(); }}
        onDrop={e => { if (e.target === e.currentTarget) { e.preventDefault(); finishDrop('root', null, 'into'); } }}
      >
        {showArchived ? (
          archivedNotes.length === 0
            ? <p className="text-gray-600 text-xs text-center mt-10 px-4">Keine archivierten Notizen.</p>
            : archivedNotes.sort(sortNotes).map(n => renderRow(n, 0))
        ) : (
          <>
            {globalPinned.length > 0 && (<><div className="px-4 pt-3 pb-1 text-gray-600 text-[10px] font-semibold uppercase tracking-widest">Angepinnt</div>{globalPinned.map(n => renderRow(n, 0))}</>)}
            {childFolders(null).map(f => renderFolder(f, 0))}
            {treeNotesIn(null).map(n => renderRow(n, 0))}
            {notes.length === 0 && folders.length === 0 && <p className="text-gray-600 text-xs text-center mt-10 px-4">No notes yet.<br />Click + to create one.</p>}
          </>
        )}
      </div>

      {menu && (
        <ContextMenu
          x={menu.x} y={menu.y}
          swatches={onSetColor ? { colors: NOTE_COLORS, current: menu.note.color, onPick: c => onSetColor(menu.note.id, c) } : undefined}
          items={[
            ...(onTogglePin ? [{ label: menu.note.pinned ? 'Lösen' : 'Anpinnen', onClick: () => onTogglePin(menu.note.id, !menu.note.pinned) }] : []),
            ...(onArchive ? [{ label: menu.note.archived ? 'Wiederherstellen' : 'Archivieren', onClick: () => onArchive(menu.note.id, !menu.note.archived) }] : []),
            ...(onMoveNote ? [{ label: 'Verschieben nach', submenu: moveSubmenu(menu.note) }] : []),
            { label: 'Exportieren', onClick: () => { void exportSelected([menu.note.id], `${(getPreview(menu.note.content).slice(0, 40) || 'notiz').replace(/[/\\:]/g, '-')}.json`); } },
          ]}
          onClose={() => setMenu(null)}
        />
      )}
      {folderMenu && (
        <ContextMenu
          x={folderMenu.x} y={folderMenu.y}
          items={[
            { label: 'Neuer Unterordner', onClick: () => createAndEdit(folderMenu.folder.id) },
            { label: 'Umbenennen', onClick: () => setEditingFolder(folderMenu.folder.id) },
            { label: 'Löschen', onClick: () => onDeleteFolder?.(folderMenu.folder) },
          ]}
          onClose={() => setFolderMenu(null)}
        />
      )}
      {rootMenu && onCreateFolder && (
        <ContextMenu
          x={rootMenu.x} y={rootMenu.y}
          items={[{ label: 'Neuer Ordner', onClick: () => createAndEdit(null) }]}
          onClose={() => setRootMenu(null)}
        />
      )}
    </aside>
  );
}
