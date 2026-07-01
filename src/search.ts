export interface SearchItem {
  id: string;
  title: string;
  content: string; // HTML
  contextId?: string;
  contextLabel?: string;
}

export interface SearchResult {
  item: SearchItem;
  snippet: string;
}

/** Plain text of an HTML fragment (matches copyFormat.ts). */
export function plainText(html: string): string {
  return (new DOMParser().parseFromString(html, 'text/html').body.textContent || '')
    .replace(/\s+/g, ' ')
    .trim();
}

function snippetAround(text: string, q: string): string {
  const idx = text.toLowerCase().indexOf(q);
  if (idx < 0) return text.slice(0, 80).trim();
  const start = Math.max(0, idx - 30);
  const end = Math.min(text.length, start + 80);
  return (start > 0 ? '…' : '') + text.slice(start, end).trim() + (end < text.length ? '…' : '');
}

/**
 * Case-insensitive substring search over title + plaintext content. Title
 * matches rank before content-only matches; original order is otherwise kept.
 */
export function searchNotes(items: SearchItem[], query: string): SearchResult[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const hits: { result: SearchResult; rank: number }[] = [];
  for (const item of items) {
    const title = item.title ?? '';
    const plain = plainText(item.content ?? '');
    const inTitle = title.toLowerCase().includes(q);
    const inContent = plain.toLowerCase().includes(q);
    if (!inTitle && !inContent) continue;
    const snippet = inContent ? snippetAround(plain, q) : title;
    hits.push({ result: { item, snippet }, rank: inTitle ? 0 : 1 });
  }
  return hits
    .map((h, i) => ({ ...h, i }))
    .sort((a, b) => a.rank - b.rank || a.i - b.i)
    .map(h => h.result);
}

/** Char ranges [from, to) of every case-insensitive occurrence of `query`. */
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
