import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { api } from '../api';
import type { WorkspaceInfo } from '../syncStatus';

export default function WorkspacePicker({ contextId, onClose }: { contextId: string; onClose: () => void }) {
  const { t } = useTranslation();
  const [list, setList] = useState<WorkspaceInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.contexts.serverWorkspaces().then(setList).catch(() => setError(t('sync.loadError')));
  }, [t]);

  const pick = async (w: WorkspaceInfo) => {
    await api.contexts.bindWorkspace(contextId, w.id, w.name);
    onClose();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }} onClick={onClose}>
      <div className="w-96 rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
        <h2 className="text-gray-100 text-base font-semibold mb-3">{t('sync.pickWorkspace')}</h2>
        {error && <p className="text-red-400 text-sm" role="alert">{error}</p>}
        {!list && !error && <p className="text-gray-400 text-sm">{t('sync.loading')}</p>}
        <div className="flex flex-col gap-1">
          {(list ?? []).map(w => (
            <button key={w.id} onClick={() => pick(w)} className="text-left px-3 py-2 rounded text-sm text-gray-100 hover:bg-gray-800">
              {w.name} <span className="text-gray-500 text-xs">· {w.role}</span>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
