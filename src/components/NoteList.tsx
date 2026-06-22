import type { Note } from '../types';

interface Props {
  notes: Note[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreate: () => void;
  onDelete: (id: string) => void;
  onOpenSettings: () => void;
}

function getPreview(html: string): string {
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

export default function NoteList({ notes, selectedId, onSelect, onCreate, onDelete, onOpenSettings }: Props) {
  return (
    <aside className="w-60 shrink-0 bg-gray-950 flex flex-col h-full select-none">
      <div className="px-4 py-3 flex items-center justify-between border-b border-gray-800">
        <span className="text-gray-400 text-xs font-semibold uppercase tracking-widest">Notes</span>
        <div className="flex items-center gap-1">
          <button
            onClick={onCreate}
            className="w-6 h-6 flex items-center justify-center text-gray-400 hover:text-white hover:bg-gray-700 rounded transition-colors"
            title="New note"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
              <line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" />
            </svg>
          </button>
          <button
            onClick={onOpenSettings}
            className="w-6 h-6 flex items-center justify-center text-gray-400 hover:text-white hover:bg-gray-700 rounded transition-colors"
            title="Settings"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="12" cy="12" r="3" />
              <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
            </svg>
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto">
        {notes.length === 0 ? (
          <p className="text-gray-600 text-xs text-center mt-10 px-4">No notes yet.<br />Click + to create one.</p>
        ) : (
          notes.map(note => (
            <button
              key={note.id}
              onClick={() => onSelect(note.id)}
              className={`w-full text-left px-4 py-3 border-b border-gray-900 group relative transition-colors ${
                selectedId === note.id ? 'bg-gray-800' : 'hover:bg-gray-900'
              }`}
            >
              <div className="flex items-start gap-2">
                <div className="w-2 h-2 rounded-sm bg-yellow-300 shrink-0 mt-1.5" />
                <div className="min-w-0 flex-1">
                  <div className="text-gray-100 text-sm font-medium truncate pr-5 leading-snug">
                    {getPreview(note.content)}
                  </div>
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
          ))
        )}
      </div>
    </aside>
  );
}
