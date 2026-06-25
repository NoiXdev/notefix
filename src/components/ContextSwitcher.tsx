import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faCheck, faPlus, faServer, faGlobe, faGear, faChevronDown } from '@fortawesome/free-solid-svg-icons';
import { api } from '../api';
import type { ContextInfo } from '../contexts';
import ContextMenu, { type ContextMenuItem } from './ContextMenu';
import PromptDialog from './PromptDialog';
import SyncStatus from './SyncStatus';
import { startServerAuth } from '../serverAuth';

export default function ContextSwitcher({ onManage }: { onManage?: () => void }) {
  const { t } = useTranslation();
  const [ctx, setCtx] = useState<ContextInfo[]>([]);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const [adding, setAdding] = useState(false);
  const [addingServer, setAddingServer] = useState(false);
  const [connecting, setConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = () => { void api.contexts.list().then(setCtx); };
  // A completed server auth emits context-changed; clear the pending state then.
  useEffect(() => { refresh(); return api.onContextChanged(() => { setConnecting(false); refresh(); }); }, []);

  const labelOf = (c: ContextInfo) =>
    c.label || (c.kind === 'server' ? c.serverUrl : t('contexts.localDefault'));
  const active = ctx.find(c => c.active);

  const submitServer = async (raw: string) => {
    setAddingServer(false);
    const urlStr = raw.trim();
    if (!urlStr) return;
    setError(null);
    setConnecting(true);
    try {
      await startServerAuth(urlStr); // opens the browser; completion arrives via context-changed
    } catch {
      setConnecting(false);
      setError(t('contexts.serverError'));
    }
  };

  const items: ContextMenuItem[] = [
    ...ctx.map(c => ({
      label: labelOf(c),
      icon: c.active
        ? <FontAwesomeIcon icon={faCheck} />
        : c.kind === 'server' ? <FontAwesomeIcon icon={faGlobe} /> : undefined,
      onClick: () => { if (!c.active) void api.contexts.switch(c.id); },
    })),
    {
      label: t('contexts.add'),
      icon: <FontAwesomeIcon icon={faPlus} />,
      onClick: () => setAdding(true),
    },
    {
      label: t('contexts.addServer'),
      icon: <FontAwesomeIcon icon={faServer} />,
      onClick: () => setAddingServer(true),
    },
    ...(onManage ? [{
      label: t('contexts.manage'),
      icon: <FontAwesomeIcon icon={faGear} />,
      onClick: onManage,
    }] : []),
  ];

  return (
    <>
      <button
        aria-label={t('contexts.switch')}
        title={t('contexts.switch')}
        onClick={e => { setError(null); setMenu({ x: e.clientX, y: e.clientY }); }}
        className="w-full flex items-center justify-between gap-1.5 px-2 py-1 rounded text-xs text-gray-400 hover:text-white hover:bg-gray-800 transition-colors"
      >
        <span className="truncate flex items-center gap-1.5">
          {active?.kind === 'server' && <FontAwesomeIcon icon={faGlobe} className="text-[10px] shrink-0" />}
          {connecting ? t('contexts.connecting') : (active ? labelOf(active) : t('contexts.localDefault'))}
        </span>
        <FontAwesomeIcon icon={faChevronDown} className="text-[10px] shrink-0 text-gray-500" />
      </button>
      {error && <div className="px-2 pb-1 text-[10px] text-red-400" role="alert">{error}</div>}
      <SyncStatus />
      {menu && <ContextMenu x={menu.x} y={menu.y} items={items} onClose={() => setMenu(null)} />}
      {adding && (
        <PromptDialog
          title={t('contexts.add')}
          confirmLabel={t('contexts.add')}
          placeholder={t('contexts.addPrompt')}
          onSubmit={name => { setAdding(false); void api.contexts.add(name); }}
          onCancel={() => setAdding(false)}
        />
      )}
      {addingServer && (
        <PromptDialog
          title={t('contexts.addServer')}
          confirmLabel={t('contexts.addServer')}
          placeholder={t('contexts.addServerPrompt')}
          onSubmit={submitServer}
          onCancel={() => setAddingServer(false)}
        />
      )}
    </>
  );
}
