import { useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faMagnifyingGlass } from '@fortawesome/free-solid-svg-icons';
import { api } from '../api';
import { getPreview } from '../preview';
import { searchNotes, type SearchItem } from '../search';

export type SearchScope = 'context' | 'global';

/** Note finder: filter notes by title + content, scoped to the active context or all. */
export default function SearchModal({ scope, onScope, onClose, onOpenNote }: {
  scope: SearchScope;
  onScope: (s: SearchScope) => void;
  onClose: () => void;
  onOpenNote: (id: string, contextId?: string) => void;
}) {
  const { t } = useTranslation();
  const [items, setItems] = useState<SearchItem[]>([]);
  const [query, setQuery] = useState('');
  const [sel, setSel] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => { inputRef.current?.focus(); }, []);

  useEffect(() => {
    let cancelled = false;
    const load: Promise<SearchItem[]> = scope === 'global'
      ? api.notes.loadAll().then(list => list.map(c => ({
          id: c.note.id, title: getPreview(c.note.content), content: c.note.content,
          contextId: c.contextId, contextLabel: c.contextLabel,
        })))
      : api.notes.load().then(list => list.map(n => ({ id: n.id, title: getPreview(n.content), content: n.content })));
    void load.then(r => { if (!cancelled) setItems(r); });
    return () => { cancelled = true; };
  }, [scope]);

  const results = useMemo(() => searchNotes(items, query).slice(0, 50), [items, query]);
  useEffect(() => { setSel(0); }, [query, scope]);

  const openAt = (i: number) => {
    const r = results[i];
    if (r) { onOpenNote(r.item.id, r.item.contextId); onClose(); }
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') { e.preventDefault(); onClose(); }
    else if (e.key === 'ArrowDown') { e.preventDefault(); setSel(s => Math.min(results.length - 1, s + 1)); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); setSel(s => Math.max(0, s - 1)); }
    else if (e.key === 'Enter') { e.preventDefault(); openAt(sel); }
  };

  return (
    <div className="fixed inset-0 z-[100] flex items-start justify-center bg-black/30 pt-[12vh]" onClick={onClose}>
      <div className="w-full max-w-lg overflow-hidden rounded-xl bg-white shadow-2xl" onClick={e => e.stopPropagation()}>
        <div className="flex items-center gap-2 border-b border-gray-100 px-3 py-2.5">
          <FontAwesomeIcon icon={faMagnifyingGlass} className="text-sm text-gray-400" />
          <input
            ref={inputRef}
            value={query}
            onChange={e => setQuery(e.target.value)}
            onKeyDown={onKeyDown}
            placeholder={t('search.placeholder')}
            className="flex-1 bg-transparent text-sm text-gray-800 outline-none"
          />
          <div className="flex rounded-md bg-gray-100 p-0.5 text-xs">
            {(['context', 'global'] as SearchScope[]).map(s => (
              <button
                key={s}
                onClick={() => onScope(s)}
                className={`rounded px-2 py-0.5 ${scope === s ? 'bg-white shadow-sm text-gray-800' : 'text-gray-500'}`}
              >
                {t(`search.scope.${s}`)}
              </button>
            ))}
          </div>
        </div>

        <ul className="max-h-[50vh] overflow-auto py-1">
          {query && results.length === 0 && (
            <li className="px-4 py-6 text-center text-sm text-gray-400">{t('search.noResults')}</li>
          )}
          {results.map((r, i) => (
            <li key={`${r.item.contextId ?? ''}:${r.item.id}`}>
              <button
                onMouseEnter={() => setSel(i)}
                onClick={() => openAt(i)}
                className={`flex w-full flex-col items-start gap-0.5 px-4 py-2 text-left ${i === sel ? 'bg-yellow-100' : 'hover:bg-gray-50'}`}
              >
                <span className="flex w-full items-center justify-between gap-2">
                  <span className="truncate text-sm font-medium text-gray-800">{r.item.title || t('search.untitled')}</span>
                  {r.item.contextLabel && <span className="shrink-0 text-[11px] text-gray-400">{r.item.contextLabel}</span>}
                </span>
                <span className="line-clamp-1 text-xs text-gray-500">{r.snippet}</span>
              </button>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
