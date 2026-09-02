import { describe, it, expect } from 'vitest';
import { LinkPreview } from './LinkPreviewNode';

// LinkPreview is a Tiptap `Node.create({...})` config object; `.config` holds
// the raw config functions verbatim (Tiptap's Extendable stores it as-is), so
// they can be exercised directly without a live ProseMirror EditorView.
type NodeConfig = {
  addAttributes: () => Record<string, { default: unknown }>;
  parseHTML: () => Array<{ tag: string; priority: number; getAttrs: (el: Element) => Record<string, unknown> }>;
  renderHTML: (arg: { node: { attrs: Record<string, unknown> }; HTMLAttributes: Record<string, unknown> }) => unknown;
};
const config = (LinkPreview as unknown as { config: NodeConfig }).config;

describe('LinkPreview node config — addAttributes', () => {
  it('defines every attribute with its default value', () => {
    const attrs = config.addAttributes();
    expect(attrs.href.default).toBe('');
    expect(attrs.display.default).toBe('card');
    expect(attrs.title.default).toBe('');
    expect(attrs.description.default).toBe('');
    expect(attrs.image.default).toBe('');
    expect(attrs.site.default).toBe('');
  });
});

describe('LinkPreview node config — parseHTML', () => {
  it('matches a[data-link-preview] with a priority beating the Link mark', () => {
    const [rule] = config.parseHTML();
    expect(rule.tag).toBe('a[data-link-preview]');
    expect(rule.priority).toBe(100);
  });

  it('reads href/display/title/description/image/site off the element', () => {
    const el = document.createElement('a');
    el.setAttribute('href', 'https://ex.com/x');
    el.setAttribute('data-display', 'inline');
    el.setAttribute('data-title', 'Titel');
    el.setAttribute('data-description', 'Desc');
    el.setAttribute('data-image', 'https://ex.com/i.png');
    el.setAttribute('data-site', 'ex.com');
    const [rule] = config.parseHTML();
    expect(rule.getAttrs(el)).toEqual({
      href: 'https://ex.com/x', display: 'inline', title: 'Titel', description: 'Desc',
      image: 'https://ex.com/i.png', site: 'ex.com',
    });
  });

  it('falls back to empty strings and a "card" display when data attrs are missing', () => {
    const el = document.createElement('a');
    el.setAttribute('href', 'https://ex.com/x');
    const [rule] = config.parseHTML();
    expect(rule.getAttrs(el)).toEqual({
      href: 'https://ex.com/x', display: 'card', title: '', description: '', image: '', site: '',
    });
  });
});

describe('LinkPreview node config — renderHTML', () => {
  it('emits an <a data-link-preview> with data-* attrs and the title as content', () => {
    const out = config.renderHTML({
      node: { attrs: { href: 'https://ex.com', display: 'inline', title: 'Titel', description: 'Desc', image: 'I', site: 'S' } },
      HTMLAttributes: {},
    });
    expect(out).toEqual([
      'a',
      { 'data-link-preview': '', href: 'https://ex.com', 'data-display': 'inline', 'data-title': 'Titel', 'data-description': 'Desc', 'data-image': 'I', 'data-site': 'S' },
      'Titel',
    ]);
  });

  it('falls back to the href as content when there is no title', () => {
    const out = config.renderHTML({
      node: { attrs: { href: 'https://ex.com', display: 'card', title: '', description: '', image: '', site: '' } },
      HTMLAttributes: {},
    });
    expect((out as unknown[])[2]).toBe('https://ex.com');
  });
});
