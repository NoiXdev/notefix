import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faChevronUp, faChevronDown, faXmark, faMagnifyingGlass } from '@fortawesome/free-solid-svg-icons';
import type { Editor } from '@tiptap/react';
import { searchState } from '../editor/searchHighlight';

/** In-note find bar: highlights matches and steps through them. */
export default function FindBar({ editor, onClose }: { editor: Editor; onClose: () => void }) {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [info, setInfo] = useState({ total: 0, current: -1 });
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => { inputRef.current?.focus(); inputRef.current?.select(); }, []);
  // Clear highlights when the bar closes.
  useEffect(() => () => { if (!editor.isDestroyed) editor.commands.clearSearch(); }, [editor]);

  const sync = () => { const s = searchState(editor); setInfo({ total: s.matches.length, current: s.current }); };
  const run = (q: string) => { setQuery(q); editor.commands.setSearch(q); sync(); };
  const step = (dir: number) => { editor.commands.stepSearch(dir); sync(); };

  const label = info.total > 0 ? `${info.current + 1}/${info.total}` : (query ? t('search.noMatches') : '');

  return (
    <div className="absolute left-1/2 top-3 z-20 flex -translate-x-1/2 items-center gap-1 rounded-lg border border-yellow-300 bg-white px-2 py-1 shadow-md">
      <FontAwesomeIcon icon={faMagnifyingGlass} className="text-xs text-gray-400" />
      <input
        ref={inputRef}
        value={query}
        onChange={e => run(e.target.value)}
        onKeyDown={e => {
          if (e.key === 'Enter') { e.preventDefault(); step(e.shiftKey ? -1 : 1); }
          else if (e.key === 'Escape') { e.preventDefault(); onClose(); }
        }}
        placeholder={t('search.findPlaceholder')}
        className="w-40 bg-transparent text-sm text-gray-800 outline-none"
      />
      <span className="min-w-[3rem] text-right text-xs text-gray-400">{label}</span>
      <button onClick={() => step(-1)} disabled={!info.total} aria-label={t('search.prev')} title={t('search.prev')} className="p-1 text-gray-500 hover:text-gray-800 disabled:opacity-40"><FontAwesomeIcon icon={faChevronUp} className="text-xs" /></button>
      <button onClick={() => step(1)} disabled={!info.total} aria-label={t('search.next')} title={t('search.next')} className="p-1 text-gray-500 hover:text-gray-800 disabled:opacity-40"><FontAwesomeIcon icon={faChevronDown} className="text-xs" /></button>
      <button onClick={onClose} aria-label={t('search.close')} title={t('search.close')} className="p-1 text-gray-500 hover:text-gray-800"><FontAwesomeIcon icon={faXmark} className="text-xs" /></button>
    </div>
  );
}
