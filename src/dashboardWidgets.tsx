import type { ReactNode } from 'react';
import type { Note, Stats } from './types';
import { getPreview } from './preview';
import { formatDate } from './dates';

export interface WidgetCtx {
  notes: Note[];
  stats: Stats | null;
  onSelectNote: (id: string) => void;
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
};

export const WIDGET_KEYS = Object.keys(WIDGETS);
