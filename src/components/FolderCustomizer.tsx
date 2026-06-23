import { useEffect, useState } from 'react';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { searchIcons, FA_BY_NAME, EMOJIS } from '../folderIcons';
import { NOTE_COLORS } from '../colors';
import type { Folder } from '../types';

interface Props {
  x: number;
  y: number;
  folder: Folder;
  onSetIcon: (icon: string) => void;
  onSetColor: (color: string) => void;
  onClose: () => void;
}

export default function FolderCustomizer({ x, y, folder, onSetIcon, onSetColor, onClose }: Props) {
  const [query, setQuery] = useState('');

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose(); };
    window.addEventListener('click', onClose);
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('click', onClose);
      window.removeEventListener('keydown', onKey);
    };
  }, [onClose]);

  const results = searchIcons(query);

  return (
    <div
      className="fixed z-50 w-64 p-3 rounded-md bg-gray-900 border border-gray-700 shadow-lg text-gray-200"
      style={{ left: Math.min(x, window.innerWidth - 270), top: Math.min(y, window.innerHeight - 380) }}
      onClick={e => e.stopPropagation()}
      onContextMenu={e => { e.preventDefault(); e.stopPropagation(); }}
    >
      <div className="text-xs font-semibold text-gray-400 mb-1">Icon</div>
      <input
        autoFocus
        value={query}
        onChange={e => setQuery(e.target.value)}
        placeholder="Icon suchen…"
        className="w-full bg-gray-800 text-gray-100 text-sm px-2 py-1 rounded outline-none mb-2"
      />
      <div className="grid grid-cols-7 gap-1 max-h-28 overflow-y-auto mb-2">
        <button onClick={() => onSetIcon('')} title="Standard" aria-label="Standard" className="h-7 flex items-center justify-center rounded hover:bg-gray-700 text-gray-400 text-xs">∅</button>
        {results.map(name => (
          <button key={name} onClick={() => onSetIcon('fa:' + name)} title={name} aria-label={name} className="h-7 flex items-center justify-center rounded hover:bg-gray-700">
            <FontAwesomeIcon icon={FA_BY_NAME[name]} />
          </button>
        ))}
      </div>
      <div className="grid grid-cols-7 gap-1 mb-2">
        {EMOJIS.map(em => (
          <button key={em} onClick={() => onSetIcon(em)} className="h-7 flex items-center justify-center rounded hover:bg-gray-700">{em}</button>
        ))}
      </div>
      <input
        onKeyDown={e => { if (e.key === 'Enter') { const v = (e.target as HTMLInputElement).value.trim(); if (v) onSetIcon(v); } }}
        placeholder="oder Emoji… ⏎"
        className="w-full bg-gray-800 text-gray-100 text-sm px-2 py-1 rounded outline-none mb-3"
      />
      <div className="text-xs font-semibold text-gray-400 mb-1">Farbe</div>
      <div className="flex items-center gap-1.5">
        <button onClick={() => onSetColor('')} aria-label="Keine Farbe" title="Keine" className="w-5 h-5 rounded-full border border-gray-600 text-gray-400 flex items-center justify-center text-[10px]">×</button>
        {NOTE_COLORS.map(c => (
          <button key={c} onClick={() => onSetColor(c)} aria-label={`Farbe ${c}`} className="w-5 h-5 rounded-full" style={{ background: c, outline: folder.color === c ? '2px solid #fff' : 'none', outlineOffset: '1px' }} />
        ))}
      </div>
    </div>
  );
}
