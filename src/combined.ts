import type { NoteMeta } from './types';

/** A note's list metadata plus the context it belongs to (Rust TaggedMeta shape). */
export interface CombinedNote {
  contextId: string;
  contextLabel: string;
  kind: 'local' | 'server';
  note: NoteMeta;
}

/** A search hit within the active context (Rust SearchHit shape). */
export interface SearchHit {
  note: NoteMeta;
  snippet: string;
}

/** A search hit tagged with its context (Rust TaggedHit shape). */
export interface CombinedHit {
  contextId: string;
  contextLabel: string;
  kind: 'local' | 'server';
  note: NoteMeta;
  snippet: string;
}

/** Deterministic, stable badge color derived from the context id. */
export function badgeColor(contextId: string): string {
  let h = 0;
  for (let i = 0; i < contextId.length; i++) h = (h * 31 + contextId.charCodeAt(i)) >>> 0;
  return `hsl(${h % 360}, 55%, 45%)`;
}
