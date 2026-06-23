import type { ReactNode } from 'react';
import type { Note, Stats, Folder } from './types';
import { getPreview } from './preview';
import { formatDate } from './dates';

export interface WidgetCtx {
  notes: Note[];
  folders: Folder[];
  stats: Stats | null;
  onSelectNote: (id: string) => void;
  onCreateNote: () => void;
}

function NoteLine({ note, onSelectNote }: { note: Note; onSelectNote: (id: string) => void }) {
  return (
    <button onClick={() => onSelectNote(note.id)} className="w-full text-left px-2 py-1 rounded hover:bg-gray-100 text-sm truncate text-gray-800">
      {getPreview(note.content)}
    </button>
  );
}

export const WIDGETS: Record<string, { label: string; render: (ctx: WidgetCtx) => ReactNode }> = {
  recent: {
    label: 'Zuletzt bearbeitet',
    render: ({ notes, onSelectNote }) => {
      const list = notes.filter(n => !n.archived).slice().sort((a, b) => b.updatedAt - a.updatedAt).slice(0, 8);
      return <div>{list.length === 0 ? <p className="text-xs text-gray-400 px-2">Keine Notizen.</p> : list.map(n => <NoteLine key={n.id} note={n} onSelectNote={onSelectNote} />)}</div>;
    },
  },
  due: {
    label: 'Anstehende Fälligkeiten',
    render: ({ notes, onSelectNote }) => {
      const list = notes.filter(n => !n.archived && n.dueAt != null).sort((a, b) => a.dueAt! - b.dueAt!).slice(0, 8);
      return <div>{list.length === 0 ? <p className="text-xs text-gray-400 px-2">Keine Fälligkeiten.</p> : list.map(n => (
        <button key={n.id} onClick={() => onSelectNote(n.id)} className="w-full text-left px-2 py-1 rounded hover:bg-gray-100 text-sm flex justify-between gap-2 text-gray-800">
          <span className="truncate">{getPreview(n.content)}</span>
          <span className="shrink-0 text-xs" style={{ color: n.dueAt! < Date.now() ? '#b91c1c' : '#92400e' }}>{formatDate(n.dueAt!, 'de')}</span>
        </button>
      ))}</div>;
    },
  },
  stats: {
    label: 'Statistik',
    render: ({ stats }) => stats ? (
      <div className="grid grid-cols-2 gap-2 text-gray-800">
        <div><div className="text-xs text-gray-500">Notizen</div><div className="text-xl font-bold">{stats.notes}</div></div>
        <div><div className="text-xs text-gray-500">Archiviert</div><div className="text-xl font-bold">{stats.archived}</div></div>
        <div><div className="text-xs text-gray-500">Zeichen</div><div className="text-xl font-bold">{stats.characters}</div></div>
        <div><div className="text-xs text-gray-500">Wörter</div><div className="text-xl font-bold">{stats.words}</div></div>
      </div>
    ) : <p className="text-xs text-gray-400 px-2">…</p>,
  },
  pinned: {
    label: 'Angepinnt',
    render: ({ notes, onSelectNote }) => {
      const list = notes.filter(n => !n.archived && n.pinned);
      return <div>{list.length === 0 ? <p className="text-xs text-gray-400 px-2">Nichts angepinnt.</p> : list.map(n => <NoteLine key={n.id} note={n} onSelectNote={onSelectNote} />)}</div>;
    },
  },
  folders: {
    label: 'Ordner-Übersicht',
    render: ({ folders, notes }) => {
      const count = (fid: string) => notes.filter(n => !n.archived && n.folderId === fid).length;
      const list = folders.slice().sort((a, b) => a.position - b.position);
      return <div>{list.length === 0 ? <p className="text-xs text-gray-400 px-2">Keine Ordner.</p> : list.map(f => (
        <div key={f.id} className="flex items-center justify-between px-2 py-1 text-sm text-gray-800">
          <span className="truncate">{f.name}</span>
          <span className="text-xs text-gray-500">{count(f.id)}</span>
        </div>
      ))}</div>;
    },
  },
  quicknote: {
    label: 'Schnellnotiz',
    render: ({ onCreateNote }) => (
      <button onClick={onCreateNote} className="w-full py-2 rounded text-sm font-medium" style={{ background: '#fde047', color: '#1c1917' }}>+ Neue Notiz</button>
    ),
  },
};

export const WIDGET_KEYS = Object.keys(WIDGETS);

export interface WidgetSize { w: number; h: number; minW: number; minH: number; }

/** Default-/Mindestgröße je Widget in Rastereinheiten (12-Spalten-Grid). */
export const WIDGET_SIZES: Record<string, WidgetSize> = {
  recent: { w: 6, h: 4, minW: 3, minH: 2 },
  due: { w: 6, h: 4, minW: 3, minH: 2 },
  stats: { w: 4, h: 3, minW: 3, minH: 2 },
  pinned: { w: 4, h: 3, minW: 2, minH: 2 },
  folders: { w: 4, h: 3, minW: 2, minH: 2 },
  quicknote: { w: 3, h: 2, minW: 2, minH: 2 },
};
