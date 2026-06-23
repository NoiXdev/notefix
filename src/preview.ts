export function getPreview(html: string): string {
  const el = document.createElement('div');
  el.innerHTML = html;
  const first = el.firstElementChild;
  const text = first?.textContent?.trim() ?? el.textContent?.trim() ?? '';
  return text.slice(0, 60) || 'New note';
}
