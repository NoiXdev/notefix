import type { CSSProperties } from 'react';
import type { DragKind, DropMode } from './dnd';

export interface ParsedDrag { kind: DragKind; id: string; }
export interface ParsedDrop { kind: DragKind | 'root'; id: string | null; mode: DropMode; }

export function parseDragId(raw: string): ParsedDrag {
  const [kind, id] = raw.split(':');
  return { kind: kind as DragKind, id };
}

export function parseDropId(raw: string): ParsedDrop {
  const parts = raw.split(':');
  if (parts[0] === 'root') return { kind: 'root', id: null, mode: 'into' };
  return { kind: parts[0] as DragKind, id: parts[1], mode: parts[2] as DropMode };
}

export function snapLineStyle(mode: DropMode | null): CSSProperties {
  if (mode === 'before') return { boxShadow: 'inset 0 3px 0 0 #facc15' };
  if (mode === 'after') return { boxShadow: 'inset 0 -3px 0 0 #facc15' };
  return {};
}
