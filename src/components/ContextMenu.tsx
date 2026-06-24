import { useEffect, useRef, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

export interface ContextMenuItem {
  label: string;
  icon?: ReactNode;
  onClick?: () => void;
  submenu?: ContextMenuItem[];
}

export interface ContextMenuSwatches {
  colors: string[];
  current: string;
  onPick: (color: string) => void;
}

interface Props {
  x: number;
  y: number;
  items: ContextMenuItem[];
  swatches?: ContextMenuSwatches;
  onClose: () => void;
}

export default function ContextMenu({ x, y, items, swatches, onClose }: Props) {
  const { t } = useTranslation();
  const [openSub, setOpenSub] = useState<number | null>(null);

  // Keep a stable reference to onClose so the dismiss listeners are attached
  // exactly once on mount and survive re-renders (otherwise a re-render between
  // opening this menu and the dismissing event would detach the listener).
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    const close = () => onCloseRef.current();
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') close(); };
    // Defer the dismiss listeners by a tick so the click/contextmenu that
    // opened this menu doesn't immediately close it again.
    const id = setTimeout(() => {
      window.addEventListener('click', close);
      window.addEventListener('contextmenu', close);
    }, 0);
    window.addEventListener('keydown', onKey);
    return () => {
      clearTimeout(id);
      window.removeEventListener('click', close);
      window.removeEventListener('contextmenu', close);
      window.removeEventListener('keydown', onKey);
    };
  }, []);

  return (
    <div
      className="fixed z-50 w-max min-w-[7rem] max-w-72 py-1 rounded-md bg-gray-900 border border-gray-700 shadow-lg"
      style={{ left: Math.min(x, window.innerWidth - 180), top: Math.min(y, window.innerHeight - 140) }}
      onClick={e => e.stopPropagation()}
      onContextMenu={e => { e.preventDefault(); e.stopPropagation(); }}
    >
      {swatches && (
        <div className="flex items-center gap-1.5 px-3 py-2 border-b border-gray-800">
          {swatches.colors.map(c => (
            <button
              key={c}
              onClick={() => { swatches.onPick(c); onClose(); }}
              className="w-4 h-4 rounded-full"
              style={{ background: c, outline: swatches.current === c ? '2px solid #fff' : 'none', outlineOffset: '1px' }}
              aria-label={t('folder.colorLabel', { color: c })}
            />
          ))}
          <button
            onClick={() => { swatches.onPick(''); onClose(); }}
            className="w-4 h-4 rounded-full border border-gray-600 text-gray-400 flex items-center justify-center text-[10px] leading-none"
            aria-label={t('folder.noColor')}
            title={t('folder.noColorShort')}
          >×</button>
        </div>
      )}
      {items.map((item, i) =>
        item.submenu ? (
          <div key={i} className="relative" onMouseEnter={() => setOpenSub(i)}>
            <button
              onClick={() => setOpenSub(openSub === i ? null : i)}
              className="w-full text-left px-3 py-1.5 text-sm text-gray-200 hover:bg-gray-800 flex items-center gap-2.5 whitespace-nowrap"
            >
              {item.icon && <span className="w-4 flex justify-center text-gray-400 shrink-0">{item.icon}</span>}
              <span className="flex-1">{item.label}</span><span className="text-gray-500">▸</span>
            </button>
            {openSub === i && (
              <div className="absolute left-full top-0 ml-0.5 w-max min-w-[7rem] py-1 rounded-md bg-gray-900 border border-gray-700 shadow-lg max-h-72 overflow-y-auto" style={{ right: x > window.innerWidth - 360 ? '100%' : undefined, left: x > window.innerWidth - 360 ? 'auto' : '100%' }}>
                {item.submenu.map((sub, j) => (
                  <button key={j} onClick={() => { sub.onClick?.(); onClose(); }} className="w-full text-left px-3 py-1.5 text-sm text-gray-200 hover:bg-gray-800 flex items-center gap-2.5 whitespace-nowrap">
                    {sub.icon && <span className="w-4 flex justify-center text-gray-400 shrink-0">{sub.icon}</span>}
                    <span>{sub.label}</span>
                  </button>
                ))}
              </div>
            )}
          </div>
        ) : (
          <button key={i} onClick={() => { item.onClick?.(); onClose(); }} className="w-full text-left px-3 py-1.5 text-sm text-gray-200 hover:bg-gray-800 transition-colors flex items-center gap-2.5 whitespace-nowrap">
            {item.icon && <span className="w-4 flex justify-center text-gray-400 shrink-0">{item.icon}</span>}
            <span>{item.label}</span>
          </button>
        )
      )}
    </div>
  );
}
