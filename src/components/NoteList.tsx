import { useState, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { DndContext, DragOverlay, PointerSensor, useSensor, useSensors, pointerWithin, type DragStartEvent, type DragOverEvent, type DragEndEvent } from '@dnd-kit/core';
import type { Note, Folder } from '../types';
import { computeDrop, type DragKind, type DropMode } from '../dnd';
import { parseDragId, parseDropId } from '../dndkit';
import { getPreview } from '../preview';
import type { PinnedScope, FolderColorStyle } from '../hooks/useSettings';
import ContextMenu, { type ContextMenuItem } from './ContextMenu';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import type { IconDefinition } from '@fortawesome/fontawesome-svg-core';
import { faThumbtack, faBoxArchive, faRightLong, faTrash, faTrashCan, faFileExport, faPalette, faArrowDownAZ, faCheck, faFolderPlus, faPen, faTableColumns, faNoteSticky, faGear, faFolder, faMagnifyingGlass } from '@fortawesome/free-solid-svg-icons';
import ConfirmDialog from './ConfirmDialog';
import FolderCustomizer from './FolderCustomizer';
import Logo from './Logo';
import ContextSwitcher from './ContextSwitcher';
import TicTacToe from './TicTacToe';
import NoteRow from './NoteRow';
import FolderRow from './FolderRow';
import RootDropZone from './RootDropZone';
import { NOTE_COLORS } from '../colors';
import { sortNotesBy } from '../sortNotes';
import type { DateFormat } from '../dates';

interface Props {
  notes: Note[];
  folders: Folder[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreate: () => void;
  onDelete: (id: string) => void;
  onOpenSettings: () => void;
  onOpenSearch?: () => void;
  onOpenDashboard?: () => void;
  onTogglePin?: (id: string, pinned: boolean) => void;
  onArchive?: (id: string, archived: boolean) => void;
  onSetColor?: (id: string, color: string) => void;
  onMoveNote?: (id: string, folderId: string | null) => void;
  onCreateFolder?: (name: string, parentId: string | null) => Promise<string>;
  onRenameFolder?: (id: string, name: string) => void;
  onDeleteFolder?: (folder: Folder) => void;
  onReorderNotes?: (folderId: string | null, ids: string[]) => void;
  onReorderFolders?: (parentId: string | null, ids: string[]) => void;
  onSetFolderIcon?: (id: string, icon: string) => void;
  onSetFolderColor?: (id: string, color: string) => void;
  onSetFolderSort?: (id: string, sort: string) => void;
  dateFormat?: DateFormat;
  pinnedScope?: PinnedScope;
  folderColorStyle?: FolderColorStyle;
  compactTree?: boolean;
  treeProgress?: boolean;
  trashed?: Note[];
  trashEnabled?: boolean;
  onRestore?: (id: string) => void;
  onPurge?: (id: string) => void;
  onEmptyTrash?: () => void;
  onExportNote: (note: Note) => void;
  onOpenContexts?: () => void;
}

const sortNotes = (a: Note, b: Note) => Number(b.pinned) - Number(a.pinned) || a.position - b.position;

const fa = (icon: IconDefinition) => <FontAwesomeIcon icon={icon} />;

const SORT_OPTIONS: { value: string; labelKey: string }[] = [
  { value: 'manual', labelKey: 'noteList.sort.manual' },
  { value: 'titleAsc', labelKey: 'noteList.sort.titleAsc' },
  { value: 'titleDesc', labelKey: 'noteList.sort.titleDesc' },
  { value: 'updatedDesc', labelKey: 'noteList.sort.updatedDesc' },
  { value: 'updatedAsc', labelKey: 'noteList.sort.updatedAsc' },
  { value: 'dueAsc', labelKey: 'noteList.sort.dueAsc' },
];

export default function NoteList(props: Props) {
  const { t } = useTranslation();
  const {
    notes, folders, selectedId, onSelect, onCreate, onDelete, onOpenSettings, onOpenSearch, onOpenDashboard,
    onTogglePin, onArchive, onSetColor, onMoveNote, onCreateFolder, onRenameFolder, onDeleteFolder,
    onReorderNotes, onReorderFolders, onSetFolderIcon, onSetFolderColor, onSetFolderSort,
    dateFormat = 'auto', pinnedScope = 'perFolder', folderColorStyle = 'icon',
    compactTree = false, treeProgress = true,
    trashed = [], trashEnabled = true, onRestore, onPurge, onEmptyTrash, onExportNote,
    onOpenContexts,
  } = props;

  const [showGame, setShowGame] = useState(false);
  const logoClicks = useRef<number[]>([]);
  const onLogoClick = () => {
    const now = Date.now();
    logoClicks.current = [...logoClicks.current, now].filter(t => now - t < 1200);
    if (logoClicks.current.length >= 4) { logoClicks.current = []; setShowGame(true); }
  };

  const [menu, setMenu] = useState<{ x: number; y: number; note: Note } | null>(null);
  const [folderMenu, setFolderMenu] = useState<{ x: number; y: number; folder: Folder } | null>(null);
  const [view, setView] = useState<'active' | 'archived' | 'trash'>('active');
  const showArchived = view === 'archived';
  const [pendingDelete, setPendingDelete] = useState<string | null>(null);
  const [pendingPurge, setPendingPurge] = useState<string | null>(null);
  const [pendingEmpty, setPendingEmpty] = useState(false);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [editingFolder, setEditingFolder] = useState<string | null>(null);
  const [rootMenu, setRootMenu] = useState<{ x: number; y: number } | null>(null);
  const [headerMenu, setHeaderMenu] = useState<{ x: number; y: number } | null>(null);
  const [customizer, setCustomizer] = useState<{ x: number; y: number; folderId: string } | null>(null);

  const toggle = (id: string) => setExpanded(prev => { const n = new Set(prev); n.has(id) ? n.delete(id) : n.add(id); return n; });

  // Create a folder, then immediately put it into inline-rename. createFolder
  // returns the new id once the list has reloaded, so editing targets a real row.
  const createAndEdit = (parentId: string | null) => {
    if (!onCreateFolder) return;
    if (parentId) setExpanded(prev => { const n = new Set(prev); n.add(parentId); return n; });
    void onCreateFolder(t('noteList.newFolderName'), parentId).then(id => setEditingFolder(id));
  };

  const [dropHint, setDropHint] = useState<{ id: string; mode: DropMode } | null>(null);
  const [activeDrag, setActiveDrag] = useState<{ kind: DragKind; id: string } | null>(null);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 5 } }));

  const applyDrop = (draggedKind: DragKind, draggedId: string, targetKind: DragKind | 'root', targetId: string | null, mode: DropMode) => {
    const res = computeDrop({ draggedKind, draggedId, targetKind, targetId, mode, notes, folders });
    if (!res) return;
    if (res.kind === 'note') onReorderNotes?.(res.parentId, res.orderedIds);
    else onReorderFolders?.(res.parentId, res.orderedIds);
  };

  const onDragStart = (e: DragStartEvent) => setActiveDrag(parseDragId(String(e.active.id)));
  const onDragOver = (e: DragOverEvent) => {
    if (!e.over) { setDropHint(null); return; }
    const d = parseDropId(String(e.over.id));
    setDropHint(d.kind === 'root' || d.id == null ? null : { id: d.id, mode: d.mode });
  };
  const onDragEnd = (e: DragEndEvent) => {
    if (e.over) {
      const a = parseDragId(String(e.active.id));
      const t = parseDropId(String(e.over.id));
      applyDrop(a.kind, a.id, t.kind, t.id, t.mode);
    }
    setDropHint(null);
    setActiveDrag(null);
  };
  const onDragCancel = () => { setDropHint(null); setActiveDrag(null); };

  // Move-to submenu: all folders indented by depth + root.
  const moveSubmenu = (note: Note): ContextMenuItem[] => {
    const byParent = (pid: string | null): Folder[] => folders.filter(f => (f.parentId ?? null) === pid).sort((a, b) => a.position - b.position);
    const items: ContextMenuItem[] = [{ label: t('noteList.moveRoot'), icon: fa(faFolder), onClick: () => onMoveNote?.(note.id, null) }];
    const walk = (pid: string | null, depth: number) => {
      for (const f of byParent(pid)) {
        items.push({ label: `${'  '.repeat(depth)}${f.name}`, icon: fa(faFolder), onClick: () => onMoveNote?.(note.id, f.id) });
        walk(f.id, depth + 1);
      }
    };
    walk(null, 0);
    return items;
  };

  const renderRow = (note: Note, depth: number) => (
    <NoteRow
      key={note.id}
      note={note}
      depth={depth}
      selected={selectedId === note.id}
      dropMode={dropHint?.id === note.id ? dropHint.mode : null}
      dateFormat={dateFormat}
      compact={compactTree}
      showProgress={treeProgress}
      onSelect={onSelect}
      onDelete={(id) => setPendingDelete(id)}
      onContextMenu={(e, n) => setMenu({ x: e.clientX, y: e.clientY, note: n })}
    />
  );

  // active (non-archived) notes for the tree; archived rendered flat.
  const activeNotes = notes.filter(n => !n.archived);
  const archivedNotes = notes.filter(n => n.archived);

  const treeNotesIn = (fid: string | null) => {
    const sort = folders.find(f => f.id === fid)?.sort ?? 'manual';
    return sortNotesBy(
      activeNotes.filter(n => (n.folderId ?? null) === fid && (pinnedScope === 'global' ? !n.pinned : true)),
      sort,
    );
  };

  const childFolders = (pid: string | null) => folders.filter(f => (f.parentId ?? null) === pid).sort((a, b) => a.position - b.position || a.name.localeCompare(b.name));

  const renderFolder = (folder: Folder, depth: number) => {
    const open = expanded.has(folder.id);
    const count = activeNotes.filter(n => (n.folderId ?? null) === folder.id).length;
    const baseStyle: React.CSSProperties = { paddingLeft: 8 + depth * 14, paddingRight: 12 };
    let iconTint = folder.color || undefined;
    if (folderColorStyle === 'row') {
      iconTint = undefined;
      if (folder.color) baseStyle.background = folder.color + '22';
    } else if (folderColorStyle === 'bar' && folder.color) {
      baseStyle.borderLeft = `3px solid ${folder.color}`;
    }
    if (editingFolder === folder.id) {
      return (
        <input
          key={folder.id}
          autoFocus
          defaultValue={folder.name}
          onBlur={e => { onRenameFolder?.(folder.id, e.target.value.trim() || folder.name); setEditingFolder(null); }}
          onKeyDown={e => { if (e.key === 'Enter') (e.target as HTMLInputElement).blur(); if (e.key === 'Escape') setEditingFolder(null); }}
          className="w-full bg-gray-800 text-gray-100 text-sm px-2 py-2 outline-none"
          style={{ marginLeft: 8 + depth * 14 }}
        />
      );
    }
    return (
      <FolderRow
        key={folder.id}
        folder={folder}
        open={open}
        count={count}
        iconTint={iconTint}
        baseStyle={baseStyle}
        dropMode={dropHint?.id === folder.id ? dropHint.mode : null}
        onToggle={toggle}
        onContextMenu={(e, f) => setFolderMenu({ x: e.clientX, y: e.clientY, folder: f })}
      >
        {childFolders(folder.id).map(f => renderFolder(f, depth + 1))}
        {treeNotesIn(folder.id).map(n => renderRow(n, depth + 1))}
      </FolderRow>
    );
  };

  const globalPinned = pinnedScope === 'global' ? activeNotes.filter(n => n.pinned).sort(sortNotes) : [];

  return (
    <aside className="w-60 shrink-0 bg-gray-950 flex flex-col h-full select-none">
      <div className="px-4 py-3 border-b border-gray-800">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-1.5">
            <button onClick={onLogoClick} className="flex items-center" aria-label="Notefix" title="Notefix"><Logo size={18} /></button>
            <span className="text-gray-200 text-xs font-semibold uppercase tracking-widest">{view === 'archived' ? t('noteList.headerArchive') : view === 'trash' ? t('noteList.headerTrash') : 'Notefix'}</span>
          </div>
          <div className="flex items-center gap-1">
            {onOpenSearch && (
              <button onClick={onOpenSearch} className="w-6 h-6 flex items-center justify-center text-gray-400 hover:text-white hover:bg-gray-700 rounded" title={t('search.open')}>
                <FontAwesomeIcon icon={faMagnifyingGlass} className="text-[13px]" />
              </button>
            )}
            {view === 'active' && (
              <button onClick={onCreate} className="w-6 h-6 flex items-center justify-center text-gray-400 hover:text-white hover:bg-gray-700 rounded" title={t('noteList.newNote')}>
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>
              </button>
            )}
            <button onClick={e => setHeaderMenu({ x: e.clientX, y: e.clientY })} className="w-6 h-6 flex items-center justify-center text-gray-400 hover:text-white hover:bg-gray-700 rounded" title={t('noteList.more')}>
              <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor"><circle cx="5" cy="12" r="1.6" /><circle cx="12" cy="12" r="1.6" /><circle cx="19" cy="12" r="1.6" /></svg>
            </button>
          </div>
        </div>
        <div className="mt-2 -mx-1">
          <ContextSwitcher onManage={onOpenContexts} />
        </div>
      </div>

      {view !== 'trash' && (
      <DndContext sensors={sensors} collisionDetection={pointerWithin} onDragStart={onDragStart} onDragOver={onDragOver} onDragEnd={onDragEnd} onDragCancel={onDragCancel}>
        <div
          className="flex-1 overflow-y-auto"
          onContextMenu={e => { if (e.target === e.currentTarget && onCreateFolder && view === 'active') { e.preventDefault(); setRootMenu({ x: e.clientX, y: e.clientY }); } }}
        >
          {showArchived ? (
            archivedNotes.length === 0
              ? <p className="text-gray-600 text-xs text-center mt-10 px-4">{t('noteList.noArchivedNotes')}</p>
              : archivedNotes.sort(sortNotes).map(n => renderRow(n, 0))
          ) : (
            <>
              {globalPinned.length > 0 && (<><div className="px-4 pt-3 pb-1 text-gray-600 text-[10px] font-semibold uppercase tracking-widest">{t('noteList.pinned')}</div>{globalPinned.map(n => renderRow(n, 0))}</>)}
              {childFolders(null).map(f => renderFolder(f, 0))}
              {treeNotesIn(null).map(n => renderRow(n, 0))}
              {notes.length === 0 && folders.length === 0 && <p className="text-gray-600 text-xs text-center mt-10 px-4">{t('noteList.emptyTitle')}<br />{t('noteList.emptyHint')}</p>}
              {(onReorderNotes || onReorderFolders) && <RootDropZone />}
            </>
          )}
        </div>
        <DragOverlay>
          {activeDrag ? (
            <div className="px-3 py-2 rounded bg-gray-800 text-gray-100 text-sm shadow-lg max-w-56 truncate">
              {activeDrag.kind === 'note'
                ? getPreview(notes.find(n => n.id === activeDrag.id)?.content ?? '')
                : (folders.find(f => f.id === activeDrag.id)?.name ?? '')}
            </div>
          ) : null}
        </DragOverlay>
      </DndContext>
      )}

      {view === 'trash' && (
        <div className="flex-1 overflow-y-auto">
          <div className="flex items-center justify-between px-4 py-2 border-b border-gray-800">
            <span className="text-gray-500 text-xs">{t('noteList.trashCount', { count: trashed.length })}</span>
            {trashed.length > 0 && <button onClick={() => setPendingEmpty(true)} className="text-xs text-red-400 hover:text-red-300">{t('noteList.emptyTrash')}</button>}
          </div>
          {trashed.length === 0 && <p className="text-gray-600 text-xs text-center mt-10 px-4">{t('noteList.trashIsEmpty')}</p>}
          {trashed.map(n => (
            <div key={n.id} className="px-4 py-2 border-b border-gray-900 flex items-center justify-between gap-2">
              <span className="text-gray-300 text-sm truncate">{getPreview(n.content)}</span>
              <div className="flex items-center gap-3 shrink-0">
                <button onClick={() => onRestore?.(n.id)} className="text-xs text-gray-400 hover:text-white" title={t('noteList.restore')}>{t('noteList.restore')}</button>
                <button onClick={() => setPendingPurge(n.id)} className="text-xs text-gray-500 hover:text-red-400" title={t('noteList.confirm.deletePermanent')}>{t('noteList.delete')}</button>
              </div>
            </div>
          ))}
        </div>
      )}

      {menu && (
        <ContextMenu
          x={menu.x} y={menu.y}
          swatches={onSetColor ? { colors: NOTE_COLORS, current: menu.note.color, onPick: c => onSetColor(menu.note.id, c) } : undefined}
          items={[
            ...(onTogglePin ? [{ label: menu.note.pinned ? t('noteList.menu.unpin') : t('noteList.menu.pin'), icon: fa(faThumbtack), onClick: () => onTogglePin(menu.note.id, !menu.note.pinned) }] : []),
            ...(onArchive ? [{ label: menu.note.archived ? t('noteList.menu.restore') : t('noteList.menu.archive'), icon: fa(faBoxArchive), onClick: () => onArchive(menu.note.id, !menu.note.archived) }] : []),
            ...(onMoveNote ? [{ label: t('noteList.menu.moveTo'), icon: fa(faRightLong), submenu: moveSubmenu(menu.note) }] : []),
            { label: t('noteList.menu.delete'), icon: fa(faTrash), onClick: () => setPendingDelete(menu.note.id) },
            { label: t('noteList.menu.export'), icon: fa(faFileExport), onClick: () => onExportNote(menu.note) },
          ]}
          onClose={() => setMenu(null)}
        />
      )}
      {folderMenu && (
        <ContextMenu
          x={folderMenu.x} y={folderMenu.y}
          items={[
            ...((onSetFolderIcon && onSetFolderColor) ? [{ label: t('noteList.menu.customize'), icon: fa(faPalette), onClick: () => setCustomizer({ x: folderMenu.x, y: folderMenu.y, folderId: folderMenu.folder.id }) }] : []),
            ...(onSetFolderSort ? [{ label: t('noteList.menu.sortBy'), icon: fa(faArrowDownAZ), submenu: SORT_OPTIONS.map(o => ({ label: t(o.labelKey), icon: fa(folderMenu.folder.sort === o.value ? faCheck : faArrowDownAZ), onClick: () => onSetFolderSort(folderMenu.folder.id, o.value) })) }] : []),
            { label: t('noteList.menu.newSubfolder'), icon: fa(faFolderPlus), onClick: () => createAndEdit(folderMenu.folder.id) },
            { label: t('noteList.menu.rename'), icon: fa(faPen), onClick: () => setEditingFolder(folderMenu.folder.id) },
            { label: t('noteList.menu.delete'), icon: fa(faTrash), onClick: () => onDeleteFolder?.(folderMenu.folder) },
          ]}
          onClose={() => setFolderMenu(null)}
        />
      )}
      {rootMenu && onCreateFolder && (
        <ContextMenu
          x={rootMenu.x} y={rootMenu.y}
          items={[{ label: t('noteList.menu.newFolder'), icon: fa(faFolderPlus), onClick: () => createAndEdit(null) }]}
          onClose={() => setRootMenu(null)}
        />
      )}
      {headerMenu && (
        <ContextMenu
          x={headerMenu.x} y={headerMenu.y}
          items={[
            ...(onOpenDashboard ? [{ label: t('noteList.menu.dashboard'), icon: fa(faTableColumns), onClick: onOpenDashboard }] : []),
            ...((onCreateFolder && view === 'active') ? [{ label: t('noteList.menu.newFolder'), icon: fa(faFolderPlus), onClick: () => createAndEdit(null) }] : []),
            ...(view !== 'active' ? [{ label: t('noteList.menu.activeNotes'), icon: fa(faNoteSticky), onClick: () => setView('active') }] : []),
            ...(view !== 'archived' ? [{ label: t('noteList.menu.showArchive'), icon: fa(faBoxArchive), onClick: () => setView('archived') }] : []),
            ...(view !== 'trash' ? [{ label: t('noteList.menu.trash'), icon: fa(faTrashCan), onClick: () => setView('trash') }] : []),
            { label: t('noteList.menu.settings'), icon: fa(faGear), onClick: onOpenSettings },
          ]}
          onClose={() => setHeaderMenu(null)}
        />
      )}
      {customizer && onSetFolderIcon && onSetFolderColor && (() => {
        const f = folders.find(x => x.id === customizer.folderId);
        return f ? (
          <FolderCustomizer
            x={customizer.x}
            y={customizer.y}
            folder={f}
            onSetIcon={icon => onSetFolderIcon(f.id, icon)}
            onSetColor={color => onSetFolderColor(f.id, color)}
            onClose={() => setCustomizer(null)}
          />
        ) : null;
      })()}
      {pendingDelete && (
        <ConfirmDialog
          title={t('noteList.confirm.deleteTitle')}
          message={trashEnabled ? t('noteList.confirm.deleteTrashMessage') : t('noteList.confirm.deletePermanentMessage')}
          confirmLabel={trashEnabled ? t('noteList.confirm.moveToTrash') : t('noteList.confirm.deletePermanent')}
          danger={!trashEnabled}
          onConfirm={() => { onDelete(pendingDelete); setPendingDelete(null); }}
          onCancel={() => setPendingDelete(null)}
        />
      )}
      {pendingPurge && (
        <ConfirmDialog
          title={t('noteList.confirm.purgeTitle')}
          message={t('noteList.confirm.purgeMessage')}
          confirmLabel={t('noteList.confirm.deletePermanent')} danger
          onConfirm={() => { onPurge?.(pendingPurge); setPendingPurge(null); }}
          onCancel={() => setPendingPurge(null)}
        />
      )}
      {pendingEmpty && (
        <ConfirmDialog
          title={t('noteList.confirm.emptyTitle')}
          message={t('noteList.confirm.emptyMessage')}
          confirmLabel={t('noteList.confirm.empty')} danger
          onConfirm={() => { onEmptyTrash?.(); setPendingEmpty(false); }}
          onCancel={() => setPendingEmpty(false)}
        />
      )}
      {showGame && <TicTacToe onClose={() => setShowGame(false)} />}
    </aside>
  );
}
