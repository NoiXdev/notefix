import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faMagnifyingGlass } from '@fortawesome/free-solid-svg-icons';
import { api } from '../api';

export type SearchScope = 'context' | 'global';

interface Row {
  id: string;
  title: string;
  snippet: string;
  contextId?: string;
  contextLabel?: string;
}

/** Note finder: full-text search (SQLite pushdown), scoped to the active context
 *  or all contexts. Runs in Rust — content is never loaded into JS. */
export default function SearchModal({ scope, onScope, onClose, onOpenNote }: {
  scope: SearchScope;
  onScope: (s: SearchScope) => void;
  onClose: () => void;
  onOpenNote: (id: string, contextId?: string) => void;
}) {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<Row[]>([]);
  const [sel, setSel] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => { inputRef.current?.focus(); }, []);

  // Debounced search; each run supersedes the previous.
  useEffect(() => {
    const q = query.trim();
    if (!q) { setResults([]); return; }
    let cancelled = false;
    const timer = setTimeout(() => {
      const run: Promise<Row[]> = scope === 'global'
        ? api.notes.searchAll(q).then(hits => hits.map(h => ({
            id: h.note.id, title: h.note.preview, snippet: h.snippet,
            contextId: h.contextId, contextLabel: h.contextLabel,
          })))
        : api.notes.search(q).then(hits => hits.map(h => ({
            id: h.note.id, title: h.note.preview, snippet: h.snippet,
          })));
      void run.then(r => { if (!cancelled) setResults(r); });
    }, 120);
    return () => { cancelled = true; clearTimeout(timer); };
  }, [query, scope]);

  useEffect(() => { setSel(0); }, [query, scope]);

  const openAt = (i: number) => {
    const r = results[i];
    if (r) { onOpenNote(r.id, r.contextId); onClose(); }
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
          {query.trim() && results.length === 0 && (
            <li className="px-4 py-6 text-center text-sm text-gray-400">{t('search.noResults')}</li>
          )}
          {results.map((r, i) => (
            <li key={`${r.contextId ?? ''}:${r.id}`}>
              <button
                onMouseEnter={() => setSel(i)}
                onClick={() => openAt(i)}
                className={`flex w-full flex-col items-start gap-0.5 px-4 py-2 text-left ${i === sel ? 'bg-yellow-100' : 'hover:bg-gray-50'}`}
              >
                <span className="flex w-full items-center justify-between gap-2">
                  <span className="truncate text-sm font-medium text-gray-800">{r.title || t('search.untitled')}</span>
                  {r.contextLabel && <span className="shrink-0 text-[11px] text-gray-400">{r.contextLabel}</span>}
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
