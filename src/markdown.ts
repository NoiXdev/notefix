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
 *  paragraph break. We give them the natural one anyway: ONE blank line between
 *  blocks is the ordinary paragraph separator, and every ADDITIONAL blank line
 *  is one empty paragraph (leading blank lines are empty paragraphs before the
 *  first block, extra trailing ones after the last). So the markdown view shows
 *  plain blank lines — no `<br>` noise — and typing blank lines there creates
 *  empty paragraphs in the note.
 *
 *  Mechanics: `markBlankLines` turns each empty top-level paragraph into a
 *  marker <br>; this rule emits a placeholder for it, because turndown caps the
 *  separator between blocks at two newlines and would swallow a third;
 *  `htmlToMarkdown` then turns each placeholder into that extra newline, and
 *  `expandBlankLines` reads the extra newlines back into `<p></p>` blocks before
 *  marked sees the markdown. Without all this, switching to the markdown view
 *  and back silently dropped every blank line. */
const BLANK_LINE_ATTR = 'data-blank-line';
const BLANK_TOKEN = '';

td.addRule('blankLine', {
  filter: (node) => node.nodeName === 'BR' && (node as HTMLElement).getAttribute(BLANK_LINE_ATTR) !== null,
  replacement: () => `\n\n${BLANK_TOKEN}\n\n`,
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
  return td
    .turndown(source)
    // Each placeholder (plus the separator turndown put after it) becomes one
    // extra newline — see the blank-line contract above. Between two blocks k
    // empty paragraphs thus read as k+2 newlines; leading ones as k newlines.
    .replace(new RegExp(`${BLANK_TOKEN}(?:\\n\\n)?`, 'g'), '\n')
    .replace(/^(-|\*|\+)\s{3,}/gm, '$1 ');
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

/** Reads the blank-line contract back: leading blank lines and every newline
 *  beyond the two of an ordinary block separator become `<p></p>` blocks, which
 *  marked passes through as raw HTML and Tiptap parses as empty paragraphs.
 *  Fenced code is left alone — blank lines inside it are content, not
 *  paragraphs. */
function expandBlankLines(md: string): string {
  const empty = (n: number) => '<p></p>\n\n'.repeat(n);
  return md
    .split(/(```[\s\S]*?```|~~~[\s\S]*?~~~)/)
    .map((segment, i) => {
      if (i % 2 === 1) return segment; // a fenced code block — untouched
      let out = segment;
      // Only the document's own start counts as "leading": after a fence the
      // newlines are an ordinary separator and fall under the rule below.
      if (i === 0) out = out.replace(/^\n+/, m => empty(m.length));
      return out.replace(/\n{3,}/g, m => '\n\n' + empty(m.length - 2));
    })
    .join('');
}

/** A literal `<br>` line in the markdown (the representation an earlier version
 *  wrote, or one the user typed) reaches us as a top-level <br> element and
 *  still becomes an empty paragraph. <br>s inside a paragraph — the line breaks
 *  from `breaks: true` — are untouched. */
function restoreBlankLines(html: string): string {
  const el = document.createElement('div');
  el.innerHTML = html;
  Array.from(el.children).forEach(child => {
    if (child.nodeName === 'BR') child.replaceWith(document.createElement('p'));
  });
  return el.innerHTML;
}

export function markdownToHtml(md: string): string {
  const html = marked.parse(expandBlankLines(md || ''), { gfm: true, breaks: true, async: false }) as string;
  return restoreLinkPreviews(fixTaskLists(restoreBlankLines(html)));
}
