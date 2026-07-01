/** Char ranges [from, to) of every case-insensitive occurrence of `query`.
 *  Used by the in-note find highlight (note-finder search runs in Rust). */
export function findMatches(text: string, query: string): Array<[number, number]> {
  const q = query.toLowerCase();
  if (!q) return [];
  const lower = text.toLowerCase();
  const out: Array<[number, number]> = [];
  let i = 0;
  while ((i = lower.indexOf(q, i)) !== -1) {
    out.push([i, i + q.length]);
    i += q.length;
  }
  return out;
}
