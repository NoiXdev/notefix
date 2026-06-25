import type { Note } from './types';

/** A note plus the context it belongs to (matches the Rust TaggedNote wire shape). */
export interface CombinedNote {
  contextId: string;
  contextLabel: string;
  kind: 'local' | 'server';
  note: Note;
}

/** Deterministic, stable badge color derived from the context id. */
export function badgeColor(contextId: string): string {
  let h = 0;
  for (let i = 0; i < contextId.length; i++) h = (h * 31 + contextId.charCodeAt(i)) >>> 0;
  return `hsl(${h % 360}, 55%, 45%)`;
}
