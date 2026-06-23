import type { CSSProperties, MouseEvent, ReactNode } from 'react';
import { useDraggable, useDroppable } from '@dnd-kit/core';
import type { Folder } from '../types';
import type { DropMode } from '../dnd';
import { snapLineStyle } from '../dndkit';
import FolderIcon from './FolderIcon';

interface Props {
  folder: Folder;
  open: boolean;
  count: number;
  iconTint?: string;
  baseStyle: CSSProperties;
  dropMode: DropMode | null;
  onToggle: (id: string) => void;
  onContextMenu: (e: MouseEvent, folder: Folder) => void;
  children?: ReactNode;
}

export default function FolderRow({ folder, open, count, iconTint, baseStyle, dropMode, onToggle, onContextMenu, children }: Props) {
  const { setNodeRef, listeners, attributes, isDragging } = useDraggable({ id: `folder:${folder.id}` });
  const before = useDroppable({ id: `folder:${folder.id}:before` });
  const into = useDroppable({ id: `folder:${folder.id}:into` });
  const after = useDroppable({ id: `folder:${folder.id}:after` });
  return (
    <div>
      <div className="relative">
        <div
          ref={setNodeRef}
          {...listeners}
          {...attributes}
          onClick={() => onToggle(folder.id)}
          onContextMenu={e => { e.preventDefault(); onContextMenu(e, folder); }}
          className={`flex items-center gap-1 py-2 text-gray-300 hover:bg-gray-900 cursor-pointer select-none ${dropMode === 'into' ? 'bg-gray-700 ring-1 ring-inset ring-yellow-400' : ''}`}
          style={{ ...baseStyle, opacity: isDragging ? 0.4 : 1, ...snapLineStyle(dropMode === 'into' ? null : dropMode) }}
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" style={{ transform: open ? 'rotate(90deg)' : 'none' }}><polyline points="9 6 15 12 9 18" /></svg>
          <FolderIcon icon={folder.icon} tint={iconTint} />
          <span className="text-sm font-medium truncate flex-1">{folder.name}</span>
          <span className="text-gray-600 text-xs">{count || ''}</span>
        </div>
        <div ref={before.setNodeRef} className="absolute inset-x-0 top-0 h-1/3 pointer-events-none" aria-hidden />
        <div ref={into.setNodeRef} className="absolute inset-x-0 top-1/3 h-1/3 pointer-events-none" aria-hidden />
        <div ref={after.setNodeRef} className="absolute inset-x-0 bottom-0 h-1/3 pointer-events-none" aria-hidden />
      </div>
      {open && children}
    </div>
  );
}
