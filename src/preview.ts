/** Short title for a note: the text of its first block element, first 60 chars.
 *  Mirrors the Rust `note_preview` used for list metadata. Empty content yields
 *  '' — callers supply a localized fallback. Used for optimistic list updates
 *  while typing, before the store round-trips the real preview. */
export function getPreview(html: string): string {
  const el = document.createElement('div');
  el.innerHTML = html;
  const first = el.firstElementChild;
  const text = first?.textContent?.trim() ?? el.textContent?.trim() ?? '';
  return text.slice(0, 60);
}
