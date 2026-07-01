import type { NoteMeta } from './types';

export type FolderSort = 'manual' | 'titleAsc' | 'titleDesc' | 'updatedDesc' | 'updatedAsc' | 'dueAsc';

const COMPARATORS: Record<string, (a: NoteMeta, b: NoteMeta) => number> = {
  manual: (a, b) => a.position - b.position,
  titleAsc: (a, b) => a.preview.localeCompare(b.preview),
  titleDesc: (a, b) => b.preview.localeCompare(a.preview),
  updatedDesc: (a, b) => b.updatedAt - a.updatedAt,
  updatedAsc: (a, b) => a.updatedAt - b.updatedAt,
  dueAsc: (a, b) => {
    if (a.dueAt == null && b.dueAt == null) return 0;
    if (a.dueAt == null) return 1;
    if (b.dueAt == null) return -1;
    return a.dueAt - b.dueAt;
  },
};

/** Pinned notes first, then by the chosen sort mode (unknown => manual). */
export function sortNotesBy(notes: NoteMeta[], sort: string): NoteMeta[] {
  const cmp = COMPARATORS[sort] ?? COMPARATORS.manual;
  return notes.slice().sort((a, b) => (Number(b.pinned) - Number(a.pinned)) || cmp(a, b));
}
