import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faRotate, faCloud, faTriangleExclamation, faKey } from '@fortawesome/free-solid-svg-icons';
import { api } from '../api';
import type { SyncStatus as S } from '../syncStatus';
import WorkspacePicker from './WorkspacePicker';

export default function SyncStatus() {
  const { t } = useTranslation();
  const [s, setS] = useState<S | null>(null);
  const [bindCtx, setBindCtx] = useState<string | null>(null);

  // A removed member leaves the workspace waiting for a key change. It is not
  // a sync state, but it rides along on the status — both the command and the
  // `sync-status` event carry it — so the badge costs no extra round-trip.
  const rotationPending = s?.vaultRotationPending ?? false;

  const refresh = () => { void api.contexts.syncStatus().then(setS); };
  useEffect(() => { refresh(); return api.onSyncStatus(setS); }, []);
  useEffect(() => api.onContextChanged(refresh), []);

  if (!s || s.state === 'local') return null;
  const icon = s.state === 'offline' ? faTriangleExclamation : s.state === 'syncing' ? faRotate : faCloud;
  const label =
    s.state === 'unbound' ? t('sync.unbound') :
    s.state === 'syncing' ? t('sync.syncing') :
    s.state === 'offline' ? t('sync.offline') : t('sync.synced');

  // An unbound server context has nothing to sync: the badge reopens the
  // workspace picker instead, which App only raises once per context switch.
  const unbound = s.state === 'unbound';
  const onClick = unbound
    ? () => void api.contexts.list().then(cs => {
        const active = cs.find(c => c.active);
        if (active) setBindCtx(active.id);
      })
    : () => api.contexts.syncNow();

  return (
    <>
      <button onClick={onClick} title={unbound ? t('sync.pickWorkspace') : t('sync.syncNow')}
        className="w-full flex items-center gap-1.5 px-2 py-0.5 text-[10px] text-gray-400 hover:text-white">
        <FontAwesomeIcon icon={icon} className={s.state === 'syncing' ? 'animate-spin' : ''} />
        <span className="truncate">{label}{s.pending > 0 ? ` · ${s.pending}` : ''}</span>
      </button>
      {rotationPending && (
        <div role="status" className="w-full flex items-center gap-1.5 px-2 py-0.5 text-[10px] text-amber-400">
          <FontAwesomeIcon icon={faKey} />
          <span className="truncate">{t('vault.rotation.pending')}</span>
        </div>
      )}
      {bindCtx && <WorkspacePicker contextId={bindCtx} onClose={() => setBindCtx(null)} />}
    </>
  );
}
