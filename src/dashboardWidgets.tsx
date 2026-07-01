import type { ReactNode } from 'react';
import { useEffect, useState } from 'react';
import type { NoteMeta, Stats, Folder } from './types';
import { formatDate } from './dates';
import { monthGrid, weekdayShorts } from './calendarGrid';
import i18n from './i18n';

export interface WidgetCtx {
  notes: NoteMeta[];
  folders: Folder[];
  stats: Stats | null;
  onSelectNote: (id: string) => void;
  onCreateNote: () => void;
}

function NoteLine({ note, onSelectNote }: { note: NoteMeta; onSelectNote: (id: string) => void }) {
  return (
    <button onClick={() => onSelectNote(note.id)} className="w-full text-left px-2 py-1 rounded hover:bg-gray-100 text-sm truncate text-gray-800">
      {note.preview || i18n.t('noteList.untitled')}
    </button>
  );
}

function ClockWidget() {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => { const id = setInterval(() => setNow(new Date()), 1000); return () => clearInterval(id); }, []);
  return <div className="text-3xl font-bold text-gray-800 tabular-nums">{now.toLocaleTimeString()}</div>;
}

function DateWidget() {
  const d = new Date();
  return (
    <div className="text-gray-800">
      <div className="text-sm text-gray-500">{d.toLocaleDateString(undefined, { weekday: 'long' })}</div>
      <div className="text-2xl font-bold">{d.toLocaleDateString()}</div>
    </div>
  );
}

function CalendarWidget() {
  const now = new Date();
  const weeks = monthGrid(now.getFullYear(), now.getMonth());
  const today = now.getDate();
  return (
    <div className="text-gray-800 text-xs">
      <div className="font-semibold mb-1">{now.toLocaleDateString(undefined, { month: 'long', year: 'numeric' })}</div>
      <div className="grid grid-cols-7 gap-0.5 text-center">
        {weekdayShorts().map((w, i) => <div key={`h${i}`} className="text-gray-400">{w}</div>)}
        {weeks.flat().map((d, i) => <div key={i} className={d === today ? 'bg-yellow-300 rounded font-bold' : ''}>{d ?? ''}</div>)}
      </div>
    </div>
  );
}

export const WIDGETS: Record<string, { labelKey: string; render: (ctx: WidgetCtx) => ReactNode }> = {
  recent: {
    labelKey: 'dashboard.widgets.recent',
    render: ({ notes, onSelectNote }) => {
      const list = notes.filter(n => !n.archived).slice().sort((a, b) => b.updatedAt - a.updatedAt).slice(0, 8);
      return <div>{list.length === 0 ? <p className="text-xs text-gray-400 px-2">{i18n.t('dashboard.noNotes')}</p> : list.map(n => <NoteLine key={n.id} note={n} onSelectNote={onSelectNote} />)}</div>;
    },
  },
  due: {
    labelKey: 'dashboard.widgets.due',
    render: ({ notes, onSelectNote }) => {
      const list = notes.filter(n => !n.archived && n.dueAt != null).sort((a, b) => a.dueAt! - b.dueAt!).slice(0, 8);
      return <div>{list.length === 0 ? <p className="text-xs text-gray-400 px-2">{i18n.t('dashboard.noDue')}</p> : list.map(n => (
        <button key={n.id} onClick={() => onSelectNote(n.id)} className="w-full text-left px-2 py-1 rounded hover:bg-gray-100 text-sm flex justify-between gap-2 text-gray-800">
          <span className="truncate">{n.preview || i18n.t('noteList.untitled')}</span>
          <span className="shrink-0 text-xs" style={{ color: n.dueAt! < Date.now() ? '#b91c1c' : '#92400e' }}>{formatDate(n.dueAt!, 'de')}</span>
        </button>
      ))}</div>;
    },
  },
  stats: {
    labelKey: 'dashboard.widgets.stats',
    render: ({ stats }) => stats ? (
      <div className="grid grid-cols-2 gap-2 text-gray-800">
        <div><div className="text-xs text-gray-500">{i18n.t('dashboard.statNotes')}</div><div className="text-xl font-bold">{stats.notes}</div></div>
        <div><div className="text-xs text-gray-500">{i18n.t('dashboard.statArchived')}</div><div className="text-xl font-bold">{stats.archived}</div></div>
        <div><div className="text-xs text-gray-500">{i18n.t('dashboard.statCharacters')}</div><div className="text-xl font-bold">{stats.characters}</div></div>
        <div><div className="text-xs text-gray-500">{i18n.t('dashboard.statWords')}</div><div className="text-xl font-bold">{stats.words}</div></div>
      </div>
    ) : <p className="text-xs text-gray-400 px-2">…</p>,
  },
  pinned: {
    labelKey: 'dashboard.widgets.pinned',
    render: ({ notes, onSelectNote }) => {
      const list = notes.filter(n => !n.archived && n.pinned);
      return <div>{list.length === 0 ? <p className="text-xs text-gray-400 px-2">{i18n.t('dashboard.nothingPinned')}</p> : list.map(n => <NoteLine key={n.id} note={n} onSelectNote={onSelectNote} />)}</div>;
    },
  },
  folders: {
    labelKey: 'dashboard.widgets.folders',
    render: ({ folders, notes }) => {
      const count = (fid: string) => notes.filter(n => !n.archived && n.folderId === fid).length;
      const list = folders.slice().sort((a, b) => a.position - b.position);
      return <div>{list.length === 0 ? <p className="text-xs text-gray-400 px-2">{i18n.t('dashboard.noFolders')}</p> : list.map(f => (
        <div key={f.id} className="flex items-center justify-between px-2 py-1 text-sm text-gray-800">
          <span className="truncate">{f.name}</span>
          <span className="text-xs text-gray-500">{count(f.id)}</span>
        </div>
      ))}</div>;
    },
  },
  quicknote: {
    labelKey: 'dashboard.widgets.quicknote',
    render: ({ onCreateNote }) => (
      <button onClick={onCreateNote} className="w-full py-2 rounded text-sm font-medium" style={{ background: '#fde047', color: '#1c1917' }}>{i18n.t('dashboard.newNote')}</button>
    ),
  },
  clock: { labelKey: 'dashboard.widgets.clock', render: () => <ClockWidget /> },
  date: { labelKey: 'dashboard.widgets.date', render: () => <DateWidget /> },
  calendar: { labelKey: 'dashboard.widgets.calendar', render: () => <CalendarWidget /> },
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
  clock: { w: 3, h: 2, minW: 2, minH: 2 },
  date: { w: 3, h: 2, minW: 2, minH: 2 },
  calendar: { w: 4, h: 4, minW: 3, minH: 3 },
};
