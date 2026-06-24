import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faCheck, faPlus, faGear, faChevronDown } from '@fortawesome/free-solid-svg-icons';
import { api } from '../api';
import type { ContextInfo } from '../contexts';
import ContextMenu, { type ContextMenuItem } from './ContextMenu';

export default function ContextSwitcher({ onManage }: { onManage?: () => void }) {
  const { t } = useTranslation();
  const [ctx, setCtx] = useState<ContextInfo[]>([]);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);

  const refresh = () => { void api.contexts.list().then(setCtx); };
  useEffect(() => { refresh(); return api.onContextChanged(refresh); }, []);

  const labelOf = (c: ContextInfo) => c.label || t('contexts.localDefault');
  const active = ctx.find(c => c.active);

  const items: ContextMenuItem[] = [
    ...ctx.map(c => ({
      label: labelOf(c),
      icon: c.active ? <FontAwesomeIcon icon={faCheck} /> : undefined,
      onClick: () => { if (!c.active) void api.contexts.switch(c.id); },
    })),
    {
      label: t('contexts.add'),
      icon: <FontAwesomeIcon icon={faPlus} />,
      onClick: () => { const name = window.prompt(t('contexts.addPrompt')); if (name != null) void api.contexts.add(name); },
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
        onClick={e => setMenu({ x: e.clientX, y: e.clientY })}
        className="w-full flex items-center justify-between gap-1.5 px-2 py-1 rounded text-xs text-gray-400 hover:text-white hover:bg-gray-800 transition-colors"
      >
        <span className="truncate">{active ? labelOf(active) : t('contexts.localDefault')}</span>
        <FontAwesomeIcon icon={faChevronDown} className="text-[10px] shrink-0 text-gray-500" />
      </button>
      {menu && <ContextMenu x={menu.x} y={menu.y} items={items} onClose={() => setMenu(null)} />}
    </>
  );
}
