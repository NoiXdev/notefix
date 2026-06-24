import { htmlToMarkdown } from './markdown';

export type CopyFormat = 'richtext' | 'html' | 'md' | 'text';

export function selectionToCopy(html: string, format: CopyFormat): string {
  if (format === 'md') return htmlToMarkdown(html);
  if (format === 'text') return new DOMParser().parseFromString(html, 'text/html').body.textContent || '';
  return html; // 'html' (and 'richtext' fallback — handler returns early for richtext)
}
