import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faGlobe, faPlus, faMagnifyingGlass, faLock, faLockOpen } from '@fortawesome/free-solid-svg-icons';
import { api } from '../api';
import { formatDate, type DateFormat } from '../dates';
import type { CombinedNote } from '../combined';
import { badgeColor } from '../combined';
import ContextSwitcher from './ContextSwitcher';

interface Props {
  selectedId: string | null;
  activeContextId: string;
  onSelectNote: (noteId: string, contextId: string) => void;
  onCreate: () => void;
  onOpenSettings: () => void;
  onOpenSearch?: () => void;
  onOpenContexts: () => void;
  dateFormat: DateFormat;
  mobile?: boolean;
  vaultExists?: boolean;
  vaultUnlocked?: boolean;
  onLockVault?: () => void;
}

export default function CombinedNoteList({ selectedId, onSelectNote, onCreate, onOpenSearch, onOpenContexts, dateFormat, mobile, vaultExists, vaultUnlocked, onLockVault }: Props) {
  const { t } = useTranslation();
  const [items, setItems] = useState<CombinedNote[]>([]);

  const reload = () => { void api.notes.loadAll().then(setItems); };
  useEffect(() => {
    reload();
    const a = api.onNotesChanged(reload);
    const b = api.onContextChanged(reload);
    return () => { a(); b(); };
  }, []);

  const labelOf = (c: CombinedNote) => c.contextLabel || t('contexts.localDefault');

  return (
    <div
      className={`${mobile ? 'w-full' : 'w-72 shrink-0'} flex flex-col bg-gray-950 border-r border-gray-900 h-full`}
      style={{ paddingTop: 'env(safe-area-inset-top)', paddingBottom: 'env(safe-area-inset-bottom)' }}
    >
      <div className="p-2 border-b border-gray-900">
        <ContextSwitcher onManage={onOpenContexts} mobile={mobile} />
      </div>
      <div className={`flex items-center justify-between border-b border-gray-900 ${mobile ? 'px-3 py-2.5' : 'px-3 py-2'}`}>
        <span className={`text-gray-400 ${mobile ? 'text-sm' : 'text-xs'}`}>{t('combined.allContexts')}</span>
        <div className={`flex items-center ${mobile ? 'gap-1' : 'gap-2'}`}>
          {onOpenSearch && (
            <button onClick={onOpenSearch} title={t('search.open')} className={`flex items-center justify-center text-gray-400 hover:text-white ${mobile ? 'w-10 h-10 text-lg' : ''}`}>
              <FontAwesomeIcon icon={faMagnifyingGlass} />
            </button>
          )}
          {vaultExists && vaultUnlocked && (
            <button onClick={onLockVault} title={t('security.lockNow')} className={`flex items-center justify-center text-gray-400 hover:text-white ${mobile ? 'w-10 h-10 text-lg' : ''}`}>
              <FontAwesomeIcon icon={faLockOpen} />
            </button>
          )}
          <button onClick={onCreate} title={t('noteList.newNote')} className={`flex items-center justify-center text-gray-400 hover:text-white ${mobile ? 'w-10 h-10 text-lg' : ''}`}>
            <FontAwesomeIcon icon={faPlus} />
          </button>
        </div>
      </div>
      <div className="flex-1 overflow-y-auto">
        {items.length === 0 && (
          <div className="p-4 text-gray-500 text-sm">{t('noteList.emptyTitle')}</div>
        )}
        {items.map(c => (
          <button
            key={`${c.contextId}:${c.note.id}`}
            onClick={() => onSelectNote(c.note.id, c.contextId)}
            className={`w-full text-left px-3 py-3 border-b border-gray-900 transition-colors ${selectedId === c.note.id ? 'bg-gray-800' : 'hover:bg-gray-900'}`}
          >
            <div className="flex items-center gap-1.5 mb-1">
              <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium text-white" style={{ background: badgeColor(c.contextId) }}>
                {c.kind === 'server' && <FontAwesomeIcon icon={faGlobe} className="text-[8px]" />}
                {labelOf(c)}
              </span>
            </div>
            <div className="text-gray-100 text-sm font-medium truncate leading-snug">
              {c.note.protected ? (
                <span className="inline-flex items-center gap-1.5 text-gray-400">
                  <FontAwesomeIcon icon={faLock} className="text-[11px]" />
                  {c.note.title || t('noteList.untitled')}
                </span>
              ) : (
                c.note.preview || t('noteList.untitled')
              )}
            </div>
            <div className="text-gray-500 text-xs mt-0.5">{formatDate(c.note.updatedAt, dateFormat)}</div>
          </button>
        ))}
      </div>
    </div>
  );
}
