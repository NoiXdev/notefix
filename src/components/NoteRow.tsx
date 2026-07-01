import type { MouseEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { useDraggable, useDroppable } from '@dnd-kit/core';
import type { NoteMeta } from '../types';
import type { DropMode } from '../dnd';
import { snapLineStyle } from '../dndkit';
import { formatDate, type DateFormat } from '../dates';
import { DEFAULT_MARKER } from '../colors';

interface Props {
  note: NoteMeta;
  depth: number;
  selected: boolean;
  dropMode: DropMode | null;
  dateFormat: DateFormat;
  onSelect: (id: string) => void;
  onDelete: (id: string) => void;
  onContextMenu: (e: MouseEvent, note: NoteMeta) => void;
  compact?: boolean;
  showProgress?: boolean;
}

function PinIcon({ color }: { color: string }) {
  return (
    <svg width="11" height="11" viewBox="0 0 24 24" fill="currentColor" className="shrink-0 mt-1" style={{ color }} aria-hidden>
      <path d="M14 4v5l2 3v2h-5v5l-1 1-1-1v-5H4v-2l2-3V4a1 1 0 0 1-1-1h10a1 1 0 0 1-1 1z" />
    </svg>
  );
}

export default function NoteRow({ note, depth, selected, dropMode, dateFormat, onSelect, onDelete, onContextMenu, compact = false, showProgress = false }: Props) {
  const { t } = useTranslation();
  const { setNodeRef, listeners, attributes, isDragging } = useDraggable({ id: `note:${note.id}` });
  const before = useDroppable({ id: `note:${note.id}:before` });
  const after = useDroppable({ id: `note:${note.id}:after` });
  const marker = note.color || DEFAULT_MARKER;
  const tasks = { done: note.tasksDone, total: note.tasksTotal };
  return (
    <div className="relative">
      <button
        ref={setNodeRef}
        {...listeners}
        {...attributes}
        onClick={() => onSelect(note.id)}
        onContextMenu={e => { e.preventDefault(); onContextMenu(e, note); }}
        className={`w-full text-left ${compact ? 'py-1.5' : 'py-3'} border-b border-gray-900 group relative transition-colors ${selected ? 'bg-gray-800' : 'hover:bg-gray-900'}`}
        style={{ paddingLeft: 16 + depth * 14, paddingRight: 16, opacity: isDragging ? 0.4 : 1, ...snapLineStyle(dropMode) }}
      >
        <div className="flex items-start gap-2">
          {note.pinned ? <PinIcon color={marker} /> : <div className="w-2 h-2 rounded-sm shrink-0 mt-1.5" style={{ background: marker }} />}
          <div className="min-w-0 flex-1">
            <div className="text-gray-100 text-sm font-medium truncate pr-5 leading-snug">{note.preview || t('noteList.untitled')}</div>
            {!compact && (
              <div className="text-gray-500 text-xs mt-0.5 flex items-center gap-2">
                <span>{formatDate(note.updatedAt, dateFormat)}</span>
                {note.dueAt != null && (
                  <span className="inline-flex items-center gap-1 px-1.5 rounded" style={note.dueAt < Date.now() ? { background: '#fee2e2', color: '#b91c1c' } : { background: '#1f2937', color: '#9ca3af' }} title={t('noteList.due')}>
                    <svg width="9" height="9" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="3" y="4" width="18" height="18" rx="2" /><line x1="3" y1="10" x2="21" y2="10" /></svg>
                    {formatDate(note.dueAt, dateFormat)}
                  </span>
                )}
              </div>
            )}
            {showProgress && tasks.total > 0 && !compact && (
              <div className="mt-1 h-[3px] rounded-full overflow-hidden" style={{ background: '#fde68a' }}>
                <div className="h-full rounded-full" style={{ width: `${(tasks.done / tasks.total) * 100}%`, background: '#ca8a04' }} />
              </div>
            )}
          </div>
        </div>
        <span onClick={e => { e.stopPropagation(); onDelete(note.id); }} className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-600 hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity" title={t('noteList.deleteNote')}>
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><polyline points="3 6 5 6 21 6" /><path d="M19 6l-1 14H6L5 6" /></svg>
        </span>
        {showProgress && tasks.total > 0 && compact && (
          <div className="absolute left-0 right-0 bottom-0 h-[2px]" style={{ background: '#fde68a' }}>
            <div className="h-full" style={{ width: `${(tasks.done / tasks.total) * 100}%`, background: '#ca8a04' }} />
          </div>
        )}
      </button>
      <div ref={before.setNodeRef} className="absolute inset-x-0 top-0 h-1/2 pointer-events-none" aria-hidden />
      <div ref={after.setNodeRef} className="absolute inset-x-0 bottom-0 h-1/2 pointer-events-none" aria-hidden />
    </div>
  );
}
