import { Node, mergeAttributes } from '@tiptap/core';
import { ReactNodeViewRenderer, NodeViewWrapper, type NodeViewProps } from '@tiptap/react';
import { createContext, useContext, useEffect, useState } from 'react';
import { api } from '../api';
import { fetchMeta } from '../linkMeta';

export type LinkDisplay = 'url' | 'inline' | 'card';
export const LinkPreviewCtx = createContext<{ enabled: boolean; mode: LinkDisplay }>({ enabled: true, mode: 'card' });

function domainOf(href: string): string {
  try { return new URL(href).hostname.replace(/^www\./, ''); } catch { return href; }
}

export function LinkPreviewView({ node, updateAttributes, selected }: NodeViewProps) {
  const { enabled } = useContext(LinkPreviewCtx);
  const a = node.attrs as { href: string; display: LinkDisplay; title: string; description: string; image: string; site: string };
  const [, setLoading] = useState(false);

  useEffect(() => {
    if (a.title || !a.href) return;
    let alive = true;
    setLoading(true);
    void fetchMeta(a.href).then(m => { if (alive) updateAttributes({ title: m.title, description: m.description, image: m.image, site: m.site }); }).finally(() => { if (alive) setLoading(false); });
    return () => { alive = false; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [a.href]);

  const open = (e: React.MouseEvent) => { e.preventDefault(); void api.openExternal(a.href); };
  const label = a.title || a.href;
  const site = a.site || domainOf(a.href);
  const display: LinkDisplay = enabled ? a.display : 'url';

  const Switcher = () => selected && enabled ? (
    <span className="lp-switch" contentEditable={false}>
      {(['url', 'inline', 'card'] as LinkDisplay[]).map(d => (
        <button key={d} onMouseDown={e => { e.preventDefault(); updateAttributes({ display: d }); }} className={a.display === d ? 'on' : ''}>{d}</button>
      ))}
    </span>
  ) : null;

  if (display === 'url') {
    return <NodeViewWrapper as="span" className="lp-w"><a href={a.href} onClick={open} className="lp-url">{label}</a><Switcher /></NodeViewWrapper>;
  }
  if (display === 'inline') {
    return (
      <NodeViewWrapper as="span" className="lp-w">
        <a href={a.href} onClick={open} className="lp-chip" title={label}><span className="lp-chip-site">{site}</span><span className="lp-chip-title">{label}</span></a>
        <Switcher />
      </NodeViewWrapper>
    );
  }
  return (
    <NodeViewWrapper as="span" className="lp-w">
      <a href={a.href} onClick={open} className="lp-card">
        {a.image ? <img className="lp-card-img" src={a.image} alt="" /> : null}
        <span className="lp-card-body">
          <span className="lp-card-title">{label}</span>
          {a.description ? <span className="lp-card-desc">{a.description}</span> : null}
          <span className="lp-card-site">{site}</span>
        </span>
      </a>
      <Switcher />
    </NodeViewWrapper>
  );
}

export const LinkPreview = Node.create({
  name: 'linkPreview',
  group: 'inline',
  inline: true,
  atom: true,
  selectable: true,
  addStorage() { return { enabled: true as boolean, mode: 'card' as LinkDisplay }; },
  addAttributes() {
    return {
      href: { default: '' }, display: { default: 'card' },
      title: { default: '' }, description: { default: '' }, image: { default: '' }, site: { default: '' },
    };
  },
  parseHTML() {
    return [{ tag: 'a[data-link-preview]', getAttrs: el => {
      const e = el as HTMLElement;
      return {
        href: e.getAttribute('href') || '', display: (e.getAttribute('data-display') as LinkDisplay) || 'card',
        title: e.getAttribute('data-title') || '', description: e.getAttribute('data-description') || '',
        image: e.getAttribute('data-image') || '', site: e.getAttribute('data-site') || '',
      };
    } }];
  },
  renderHTML({ node, HTMLAttributes }) {
    const a = node.attrs;
    return ['a', mergeAttributes(HTMLAttributes, {
      'data-link-preview': '', href: a.href, 'data-display': a.display,
      'data-title': a.title, 'data-description': a.description, 'data-image': a.image, 'data-site': a.site,
    }), a.title || a.href];
  },
  addNodeView() { return ReactNodeViewRenderer(LinkPreviewView); },
});
