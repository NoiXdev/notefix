/** Count Tiptap task-list items in HTML: total `li[data-checked]`, done where data-checked="true". */
export function countTasks(html: string): { done: number; total: number } {
  const el = document.createElement('div');
  el.innerHTML = html;
  const items = el.querySelectorAll('li[data-checked]');
  let done = 0;
  items.forEach(li => { if (li.getAttribute('data-checked') === 'true') done++; });
  return { done, total: items.length };
}
