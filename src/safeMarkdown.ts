import { marked, Renderer, type Tokens } from 'marked';

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

// Only http(s), mailto, and same-document/relative links are allowed through.
// Anything else (javascript:, data:, vbscript:, …) is neutered to "#" —
// marked itself does NOT block dangerous URL schemes (its `cleanUrl` helper
// only percent-encodes; it does not check the scheme at all), so without
// this a release body could ship a clickable `javascript:` link.
const SAFE_URL_SCHEME = /^(https?:|mailto:)/i;
const SAFE_RELATIVE = /^[#/]/;

function sanitizeUrl(url: string): string {
  const trimmed = url.trim();
  return SAFE_URL_SCHEME.test(trimmed) || SAFE_RELATIVE.test(trimmed) ? url : '#';
}

// GitHub release bodies are external, untrusted content (unlike the note
// editor's own markdown conversion in ./markdown, which round-trips content
// this app generated itself). marked's default renderer passes raw HTML
// tokens (a literal `<script>`, an `<img onerror=…>`, etc.) straight through
// unescaped — only actual markdown constructs like **bold** or [text](url)
// go through marked's own rendering path at all. Overriding `html` to escape
// instead of pass through neutralizes any literal HTML embedded in the
// source (it renders as visible text, not markup), and overriding
// `link`/`image` to sanitize the href/src closes the URL-scheme gap above,
// while normal markdown syntax still renders normally.
const safeRenderer = new Renderer();
safeRenderer.html = ({ text }) => escapeHtml(text);
safeRenderer.link = function (this: Renderer, token: Tokens.Link): string {
  return Renderer.prototype.link.call(this, { ...token, href: sanitizeUrl(token.href) });
};
safeRenderer.image = function (this: Renderer, token: Tokens.Image): string {
  return Renderer.prototype.image.call(this, { ...token, href: sanitizeUrl(token.href) });
};

/** Render an untrusted markdown string (e.g. a GitHub release body) to HTML
 * with raw inline/block HTML escaped and link/image URLs scheme-checked, so
 * the result is safe to use with dangerouslySetInnerHTML. */
export function renderMarkdownSafe(md: string): string {
  return marked.parse(md, { renderer: safeRenderer, async: false }) as string;
}
