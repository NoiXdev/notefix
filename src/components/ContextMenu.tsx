import { useEffect } from 'react';

export interface ContextMenuItem {
  label: string;
  onClick: () => void;
}

interface Props {
  x: number;
  y: number;
  items: ContextMenuItem[];
  onClose: () => void;
}

export default function ContextMenu({ x, y, items, onClose }: Props) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') onClose(); };
    window.addEventListener('click', onClose);
    window.addEventListener('contextmenu', onClose);
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('click', onClose);
      window.removeEventListener('contextmenu', onClose);
      window.removeEventListener('keydown', onKey);
    };
  }, [onClose]);

  return (
    <div
      className="fixed z-50 min-w-32 py-1 rounded-md bg-gray-900 border border-gray-700 shadow-lg"
      style={{ left: x, top: y }}
      onClick={e => e.stopPropagation()}
      onContextMenu={e => { e.preventDefault(); e.stopPropagation(); }}
    >
      {items.map((item, i) => (
        <button
          key={i}
          onClick={() => { item.onClick(); onClose(); }}
          className="w-full text-left px-3 py-1.5 text-sm text-gray-200 hover:bg-gray-800 transition-colors"
        >
          {item.label}
        </button>
      ))}
    </div>
  );
}
