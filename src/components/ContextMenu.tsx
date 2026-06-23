import { useEffect } from 'react';

export interface ContextMenuItem {
  label: string;
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
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose(); };
    window.addEventListener('click', onClose);
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('click', onClose);
      window.removeEventListener('keydown', onKey);
    };
  }, [onClose]);

  return (
    <div
      className="fixed z-50 min-w-40 py-1 rounded-md bg-gray-900 border border-gray-700 shadow-lg"
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
              aria-label={`Farbe ${c}`}
            />
          ))}
          <button
            onClick={() => { swatches.onPick(''); onClose(); }}
            className="w-4 h-4 rounded-full border border-gray-600 text-gray-400 flex items-center justify-center text-[10px] leading-none"
            aria-label="Keine Farbe"
            title="Keine"
          >×</button>
        </div>
      )}
      {items.map((item, i) =>
        item.submenu ? (
          <div key={i} className="relative group/sub">
            <button className="w-full text-left px-3 py-1.5 text-sm text-gray-200 hover:bg-gray-800 flex items-center justify-between">
              <span>{item.label}</span><span className="text-gray-500">▸</span>
            </button>
            <div className="absolute left-full top-0 ml-0.5 min-w-40 py-1 rounded-md bg-gray-900 border border-gray-700 shadow-lg hidden group-hover/sub:block max-h-72 overflow-y-auto">
              {item.submenu.map((sub, j) => (
                <button key={j} onClick={() => { sub.onClick?.(); onClose(); }} className="w-full text-left px-3 py-1.5 text-sm text-gray-200 hover:bg-gray-800">
                  {sub.label}
                </button>
              ))}
            </div>
          </div>
        ) : (
          <button key={i} onClick={() => { item.onClick?.(); onClose(); }} className="w-full text-left px-3 py-1.5 text-sm text-gray-200 hover:bg-gray-800 transition-colors">
            {item.label}
          </button>
        )
      )}
    </div>
  );
}
