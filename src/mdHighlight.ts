import hljs from 'highlight.js/lib/core';
import markdown from 'highlight.js/lib/languages/markdown';

hljs.registerLanguage('markdown', markdown);

export function highlightMarkdown(code: string): string {
  return hljs.highlight(code, { language: 'markdown' }).value;
}
