export type DateFormat = 'auto' | 'de' | 'iso' | 'us';

const p2 = (n: number) => String(n).padStart(2, '0');

export function formatDate(ts: number, fmt: DateFormat): string {
  const d = new Date(ts);
  const yyyy = d.getFullYear();
  const mm = p2(d.getMonth() + 1);
  const dd = p2(d.getDate());
  switch (fmt) {
    case 'de': return `${dd}.${mm}.${yyyy}`;
    case 'iso': return `${yyyy}-${mm}-${dd}`;
    case 'us': return `${mm}/${dd}/${yyyy}`;
    case 'auto':
    default: {
      const now = new Date();
      if (d.toDateString() === now.toDateString()) {
        return d.toLocaleTimeString('en-US', { hour: 'numeric', minute: '2-digit' });
      }
      return d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
    }
  }
}

/** epoch ms -> "YYYY-MM-DD" for <input type="date">, or "" for null. */
export function toDateInputValue(ts: number | null): string {
  if (ts == null) return '';
  const d = new Date(ts);
  return `${d.getFullYear()}-${p2(d.getMonth() + 1)}-${p2(d.getDate())}`;
}

/** "YYYY-MM-DD" -> epoch ms at local midnight, or null for "". */
export function fromDateInputValue(v: string): number | null {
  if (!v) return null;
  const [y, m, d] = v.split('-').map(Number);
  return new Date(y, m - 1, d).getTime();
}
