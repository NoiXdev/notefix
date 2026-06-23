import { useEffect, useState } from 'react';
import { api } from '../api';
import type { Revision } from '../types';
import { formatDate } from '../dates';

interface Props {
  noteId: string;
  onRestore: (content: string) => void;
  onClose: () => void;
}

export default function HistoryModal({ noteId, onRestore, onClose }: Props) {
  const [revisions, setRevisions] = useState<Revision[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const [content, setContent] = useState('');

  useEffect(() => { api.notes.revisions(noteId).then(setRevisions); }, [noteId]);

  const pick = async (id: number) => {
    setSelected(id);
    setContent((await api.notes.revisionContent(id)) ?? '');
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40" onClick={onClose}>
      <div className="bg-gray-900 text-gray-200 rounded-lg shadow-xl w-[680px] max-w-[92vw] h-[460px] flex overflow-hidden" onClick={e => e.stopPropagation()}>
        <div className="w-56 shrink-0 border-r border-gray-800 overflow-y-auto">
          <div className="px-3 py-2 text-xs font-semibold text-gray-400 border-b border-gray-800">Verlauf</div>
          {revisions.length === 0 && <p className="px-3 py-3 text-xs text-gray-500">Noch keine Versionen.</p>}
          {revisions.map(r => (
            <button
              key={r.id}
              aria-label={`Version ${r.id}`}
              onClick={() => pick(r.id)}
              className={`w-full text-left px-3 py-2 text-xs border-b border-gray-800 hover:bg-gray-800 ${selected === r.id ? 'bg-gray-800' : ''}`}
            >
              {formatDate(r.createdAt, 'auto')}
            </button>
          ))}
        </div>
        <div className="flex-1 flex flex-col">
          <div className="flex-1 overflow-y-auto p-4 bg-yellow-50 text-gray-900">
            {selected ? <div dangerouslySetInnerHTML={{ __html: content }} /> : <p className="text-gray-400 text-sm">Wähle eine Version.</p>}
          </div>
          <div className="flex justify-end gap-2 p-3 border-t border-gray-800">
            <button onClick={onClose} className="px-3 py-1.5 rounded text-sm text-gray-300 hover:bg-gray-800">Schließen</button>
            <button disabled={selected == null} onClick={() => onRestore(content)} className="px-3 py-1.5 rounded text-sm font-medium disabled:opacity-40" style={{ background: '#fde047', color: '#1c1917' }}>Wiederherstellen</button>
          </div>
        </div>
      </div>
    </div>
  );
}
