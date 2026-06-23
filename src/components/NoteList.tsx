import { useState } from 'react';
import type { Note } from '../types';
import type { PinnedDisplayMode } from '../hooks/useSettings';
import ContextMenu from './ContextMenu';
import { NOTE_COLORS, DEFAULT_MARKER } from '../colors';
import { exportSelected } from '../export';

interface Props {
  notes: Note[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreate: () => void;
  onDelete: (id: string) => void;
  onOpenSettings: () => void;
  displayMode?: PinnedDisplayMode;
  onTogglePin?: (id: string, pinned: boolean) => void;
  onArchive?: (id: string, archived: boolean) => void;
  onSetColor?: (id: string, color: string) => void;
}

export function getPreview(html: string): string {
  const el = document.createElement('div');
  el.innerHTML = html;
  const first = el.firstElementChild;
  const text = first?.textContent?.trim() ?? el.textContent?.trim() ?? '';
  return text.slice(0, 60) || 'New note';
}

function formatDate(ts: number): string {
  const d = new Date(ts);
  const now = new Date();
  if (d.toDateString() === now.toDateString()) {
    return d.toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit' });
  }
  return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
}

function PinIcon({ color }: { color: string }) {
  return (
    <svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor" className="shrink-0 mt-1" style={{ color }} aria-hidden>
      <path d="M14 4v5l2 3v2h-5v5l-1 1-1-1v-5H4v-2l2-3V4a1 1 0 0 1-1-1h10a1 1 0 0 1-1 1z" />
    </svg>
  );
}

export default function NoteList({
  notes, selectedId, onSelect, onCreate, onDelete, onOpenSettings,
  displayMode = 'flat', onTogglePin, onArchive, onSetColor,
}: Props) {
  const [menu, setMenu] = useState<{ x: number; y: number; note: Note } | null>(null);
  const [showArchived, setShowArchived] = useState(false);

  const visible = notes.filter(n => !!n.archived === showArchived);

  const renderRow = (note: Note) => {
    const marker = note.color || DEFAULT_MARKER;
    return (
      <button
        key={note.id}
        onClick={() => onSelect(note.id)}
        onContextMenu={e => { e.preventDefault(); setMenu({ x: e.clientX, y: e.clientY, note }); }}
        className={`w-full text-left px-4 py-3 border-b border-gray-900 group relative transition-colors ${
          selectedId === note.id ? 'bg-gray-800' : 'hover:bg-gray-900'
        }`}
      >
        <div className="flex items-start gap-2">
          {note.pinned
            ? <PinIcon color={marker} />
            : <div className="w-2 h-2 rounded-sm shrink-0 mt-1.5" style={{ background: marker }} />}
          <div className="min-w-0 flex-1">
            <div className="text-gray-100 text-sm font-medium truncate pr-5 leading-snug">{getPreview(note.content)}</div>
            <div className="text-gray-500 text-xs mt-0.5">{formatDate(note.updatedAt)}</div>
          </div>
        </div>
        <button
          onClick={e => { e.stopPropagation(); onDelete(note.id); }}
          className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-600 hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity"
          title="Delete note"
        >
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <polyline points="3 6 5 6 21 6" /><path d="M19 6l-1 14H6L5 6" /><path d="M10 11v6" /><path d="M14 11v6" /><path d="M9 6V4h6v2" />
          </svg>
        </button>
      </button>
    );
  };

  const pinned = visible.filter(n => n.pinned);
  const unpinned = visible.filter(n => !n.pinned);

  const sectionHeader = (label: string) => (
    <div className="px-4 pt-3 pb-1 text-gray-600 text-[10px] font-semibold uppercase tracking-widest select-none">{label}</div>
  );

  return (
    <aside className="w-60 shrink-0 bg-gray-950 flex flex-col h-full select-none">
      <div className="px-4 py-3 flex items-center justify-between border-b border-gray-800">
        <span className="text-gray-400 text-xs font-semibold uppercase tracking-widest">{showArchived ? 'Archiv' : 'Notes'}</span>
        <div className="flex items-center gap-1">
          <button
            onClick={() => setShowArchived(v => !v)}
            className={`w-6 h-6 flex items-center justify-center rounded transition-colors ${showArchived ? 'text-white bg-gray-700' : 'text-gray-400 hover:text-white hover:bg-gray-700'}`}
            title={showArchived ? 'Aktive Notizen' : 'Archiv anzeigen'}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <rect x="3" y="4" width="18" height="4" rx="1" /><path d="M5 8v11a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V8" /><line x1="10" y1="12" x2="14" y2="12" />
            </svg>
          </button>
          {!showArchived && (
            <button onClick={onCreate} className="w-6 h-6 flex items-center justify-center text-gray-400 hover:text-white hover:bg-gray-700 rounded transition-colors" title="New note">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>
            </button>
          )}
          <button onClick={onOpenSettings} className="w-6 h-6 flex items-center justify-center text-gray-400 hover:text-white hover:bg-gray-700 rounded transition-colors" title="Settings">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="12" cy="12" r="3" />
              <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
            </svg>
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto">
        {visible.length === 0 ? (
          <p className="text-gray-600 text-xs text-center mt-10 px-4">
            {showArchived ? 'Keine archivierten Notizen.' : <>No notes yet.<br />Click + to create one.</>}
          </p>
        ) : displayMode === 'sections' ? (
          <>
            {pinned.length > 0 && (<>{sectionHeader('Angepinnt')}{pinned.map(renderRow)}</>)}
            {unpinned.length > 0 && (<>{sectionHeader('Weitere')}{unpinned.map(renderRow)}</>)}
          </>
        ) : (
          <>
            {pinned.map(renderRow)}
            {pinned.length > 0 && unpinned.length > 0 && <div className="border-t-2 border-gray-800" data-testid="pin-divider" />}
            {unpinned.map(renderRow)}
          </>
        )}
      </div>

      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          swatches={onSetColor ? { colors: NOTE_COLORS, current: menu.note.color, onPick: c => onSetColor(menu.note.id, c) } : undefined}
          items={[
            ...(onTogglePin ? [{ label: menu.note.pinned ? 'Lösen' : 'Anpinnen', onClick: () => onTogglePin(menu.note.id, !menu.note.pinned) }] : []),
            ...(onArchive ? [{ label: menu.note.archived ? 'Wiederherstellen' : 'Archivieren', onClick: () => onArchive(menu.note.id, !menu.note.archived) }] : []),
            { label: 'Exportieren', onClick: () => { void exportSelected([menu.note.id], `${getPreview(menu.note.content).slice(0, 40) || 'notiz'}.json`); } },
          ]}
          onClose={() => setMenu(null)}
        />
      )}
    </aside>
  );
}
