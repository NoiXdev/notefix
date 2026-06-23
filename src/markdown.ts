import TurndownService from 'turndown';
import { gfm } from 'turndown-plugin-gfm';
import { marked } from 'marked';

const td = new TurndownService({ headingStyle: 'atx', codeBlockStyle: 'fenced', bulletListMarker: '-' });
td.use(gfm);

td.addRule('tiptapTaskItem', {
  filter: (node) => node.nodeName === 'LI' && (node as HTMLElement).getAttribute('data-checked') !== null,
  replacement: (content, node) => {
    const checked = (node as HTMLElement).getAttribute('data-checked') === 'true';
    return `- [${checked ? 'x' : ' '}] ${content.replace(/^\s+/, '').trim()}\n`;
  },
});

export function htmlToMarkdown(html: string): string {
  return td.turndown(html || '').replace(/^(-|\*|\+)\s{3,}/gm, '$1 ');
}

function fixTaskLists(html: string): string {
  const el = document.createElement('div');
  el.innerHTML = html;
  el.querySelectorAll('li').forEach(li => {
    const input = li.querySelector('input[type="checkbox"]');
    if (!input) return;
    li.setAttribute('data-type', 'taskItem');
    li.setAttribute('data-checked', input.hasAttribute('checked') ? 'true' : 'false');
    input.remove();
    li.closest('ul')?.setAttribute('data-type', 'taskList');
  });
  return el.innerHTML;
}

export function markdownToHtml(md: string): string {
  const html = marked.parse(md || '', { gfm: true, breaks: true, async: false }) as string;
  return fixTaskLists(html);
}
