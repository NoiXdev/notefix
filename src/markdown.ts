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

/** Empty paragraphs — the blank lines you get from pressing Enter a few times —
 *  have no markdown equivalent: a run of blank lines collapses into a single
 *  paragraph break. `markBlankLines` turns each one into a marker <br> that this
 *  rule writes out as an explicit `<br>` line, which `markdownToHtml` reads back
 *  as an empty paragraph. Without it, switching to the markdown view and back
 *  silently dropped every blank line. */
const BLANK_LINE_ATTR = 'data-blank-line';

td.addRule('blankLine', {
  filter: (node) => node.nodeName === 'BR' && (node as HTMLElement).getAttribute(BLANK_LINE_ATTR) !== null,
  replacement: () => '\n\n<br>\n\n',
});

td.addRule('linkPreview', {
  filter: (node) => node.nodeName === 'A' && (node as HTMLElement).getAttribute('data-link-preview') !== null,
  replacement: (_content, node) => (node as HTMLElement).getAttribute('href') || '',
});

/** Replaces top-level empty paragraphs (also `<p><br></p>`, as pasted HTML
 *  writes them) with the marker <br> the `blankLine` rule picks up. Nested
 *  empty paragraphs — inside a list item, say — are left alone. */
function markBlankLines(html: string): string {
  const el = document.createElement('div');
  el.innerHTML = html;
  // A note that is nothing but empty paragraphs stays empty markdown.
  if (!(el.textContent || '').trim() && !el.querySelector('img')) return html;
  Array.from(el.children).forEach(child => {
    if (child.nodeName !== 'P') return;
    if ((child.textContent || '').trim() !== '') return;
    if (child.querySelector(':not(br)')) return;
    const br = document.createElement('br');
    br.setAttribute(BLANK_LINE_ATTR, '');
    child.replaceWith(br);
  });
  return el.innerHTML;
}

export interface HtmlToMarkdownOptions {
  /** Carry blank lines across as explicit `<br>` lines (see `markBlankLines`).
   *  The editor's markdown view needs it to round-trip losslessly; exports and
   *  clipboard copies leave it off, where clean markdown beats exact fidelity. */
  blankLines?: boolean;
}

export function htmlToMarkdown(html: string, opts: HtmlToMarkdownOptions = {}): string {
  const source = opts.blankLines ? markBlankLines(html || '') : (html || '');
  return td.turndown(source).replace(/^(-|\*|\+)\s{3,}/gm, '$1 ');
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

function restoreLinkPreviews(html: string): string {
  const el = document.createElement('div');
  el.innerHTML = html;
  el.querySelectorAll('a[href]').forEach(a => {
    if (a.getAttribute('data-link-preview') !== null) return;
    const href = a.getAttribute('href') || '';
    const text = (a.textContent || '').trim();
    if (/^https?:\/\//.test(href) && text === href) {
      a.setAttribute('data-link-preview', '');
      a.setAttribute('data-display', 'card');
    }
  });
  return el.innerHTML;
}

/** Counterpart of `markBlankLines`: a `<br>` line in the markdown reaches us as
 *  a top-level <br> element (marked passes HTML blocks through), and becomes the
 *  empty paragraph it came from. <br>s inside a paragraph — the line breaks from
 *  `breaks: true` — are untouched. */
function restoreBlankLines(html: string): string {
  const el = document.createElement('div');
  el.innerHTML = html;
  Array.from(el.children).forEach(child => {
    if (child.nodeName === 'BR') child.replaceWith(document.createElement('p'));
  });
  return el.innerHTML;
}

export function markdownToHtml(md: string): string {
  const html = marked.parse(md || '', { gfm: true, breaks: true, async: false }) as string;
  return restoreLinkPreviews(fixTaskLists(restoreBlankLines(html)));
}
