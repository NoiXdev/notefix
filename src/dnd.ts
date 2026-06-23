import type { Note, Folder } from './types';

export type DragKind = 'note' | 'folder';
export type DropMode = 'into' | 'before' | 'after';

export interface DropResult {
  kind: DragKind;
  parentId: string | null;
  orderedIds: string[];
}

const byPosN = (a: Note, b: Note) => a.position - b.position;
const byPosF = (a: Folder, b: Folder) => a.position - b.position;

function folderDescendants(folders: Folder[], id: string): Set<string> {
  const out = new Set<string>();
  const walk = (pid: string) => {
    for (const f of folders) if ((f.parentId ?? null) === pid && !out.has(f.id)) { out.add(f.id); walk(f.id); }
  };
  walk(id);
  return out;
}

export function computeDrop(args: {
  draggedKind: DragKind; draggedId: string;
  targetKind: DragKind | 'root'; targetId: string | null; mode: DropMode;
  notes: Note[]; folders: Folder[];
}): DropResult | null {
  const { draggedKind, draggedId, targetKind, targetId, mode, notes, folders } = args;

  if (draggedKind === 'note') {
    const place = (dest: string | null, beforeId: string | null, after: boolean): DropResult => {
      const sibs = notes.filter(n => (n.folderId ?? null) === dest && n.id !== draggedId).sort(byPosN).map(n => n.id);
      if (beforeId == null) return { kind: 'note', parentId: dest, orderedIds: [...sibs, draggedId] };
      const idx = sibs.indexOf(beforeId);
      sibs.splice(after ? idx + 1 : idx, 0, draggedId);
      return { kind: 'note', parentId: dest, orderedIds: sibs };
    };
    if (targetKind === 'folder' && mode === 'into') return place(targetId, null, false);
    if (targetKind === 'note') {
      const t = notes.find(n => n.id === targetId);
      if (!t) return null;
      return place(t.folderId ?? null, targetId, mode === 'after');
    }
    if (targetKind === 'root') return place(null, null, false);
    return null;
  }

  // folder
  const desc = folderDescendants(folders, draggedId);
  const placeF = (dest: string | null, beforeId: string | null, after: boolean): DropResult | null => {
    if (dest === draggedId || (dest != null && desc.has(dest))) return null; // cycle
    const sibs = folders.filter(f => (f.parentId ?? null) === dest && f.id !== draggedId).sort(byPosF).map(f => f.id);
    if (beforeId == null) return { kind: 'folder', parentId: dest, orderedIds: [...sibs, draggedId] };
    const idx = sibs.indexOf(beforeId);
    sibs.splice(after ? idx + 1 : idx, 0, draggedId);
    return { kind: 'folder', parentId: dest, orderedIds: sibs };
  };
  if (targetKind === 'folder' && mode === 'into') return placeF(targetId, null, false);
  if (targetKind === 'folder') {
    const t = folders.find(f => f.id === targetId);
    if (!t) return null;
    return placeF(t.parentId ?? null, targetId, mode === 'after');
  }
  if (targetKind === 'root') return placeF(null, null, false);
  return null;
}
