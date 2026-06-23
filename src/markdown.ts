import TurndownService from 'turndown';
import { gfm } from 'turndown-plugin-gfm';
import { marked } from 'marked';

const td = new TurndownService({ headingStyle: 'atx', codeBlockStyle: 'fenced', bulletListMarker: '-' });
td.use(gfm);

export function htmlToMarkdown(html: string): string {
  return td.turndown(html || '').replace(/^(-|\*|\+)\s{3,}/gm, '$1 ');
}

export function markdownToHtml(md: string): string {
  return marked.parse(md || '', { gfm: true, breaks: true, async: false }) as string;
}
