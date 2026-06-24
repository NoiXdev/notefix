import { Node, mergeAttributes } from '@tiptap/core';
import { ReactNodeViewRenderer, NodeViewWrapper, type NodeViewProps } from '@tiptap/react';
import { createContext, useContext, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { api } from '../api';
import { fetchMeta } from '../linkMeta';
import ContextMenu from './ContextMenu';

export type LinkDisplay = 'url' | 'inline' | 'card';
export const LinkPreviewCtx = createContext<{ enabled: boolean; mode: LinkDisplay }>({ enabled: true, mode: 'card' });

function domainOf(href: string): string {
  try { return new URL(href).hostname.replace(/^www\./, ''); } catch { return href; }
}

export function LinkPreviewView({ node, updateAttributes, deleteNode, selected }: NodeViewProps) {
  const { enabled } = useContext(LinkPreviewCtx);
  const { t } = useTranslation();
  const a = node.attrs as { href: string; display: LinkDisplay; title: string; description: string; image: string; site: string };
  const [, setLoading] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);

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

  const menuItems = [
    { label: t('editor.linkMenu.open'), onClick: () => { void api.openExternal(a.href); } },
    { label: t('editor.linkMenu.display'), submenu: [
      { label: t('editor.linkMenu.displayUrl'), onClick: () => updateAttributes({ display: 'url' }) },
      { label: t('editor.linkMenu.displayInline'), onClick: () => updateAttributes({ display: 'inline' }) },
      { label: t('editor.linkMenu.displayCard'), onClick: () => updateAttributes({ display: 'card' }) },
    ] },
    { label: t('editor.linkMenu.remove'), onClick: () => deleteNode() },
  ];
  const onCtx = (e: React.MouseEvent) => { e.preventDefault(); e.stopPropagation(); setMenu({ x: e.clientX, y: e.clientY }); };
  const Menu = () => menu ? <ContextMenu x={menu.x} y={menu.y} items={menuItems} onClose={() => setMenu(null)} /> : null;

  if (display === 'url') {
    return <NodeViewWrapper as="span" className="lp-w" onContextMenu={onCtx}><a href={a.href} onClick={open} className="lp-url">{label}</a><Switcher /><Menu /></NodeViewWrapper>;
  }
  if (display === 'inline') {
    return (
      <NodeViewWrapper as="span" className="lp-w" onContextMenu={onCtx}>
        <a href={a.href} onClick={open} className="lp-chip" title={label}><span className="lp-chip-site">{site}</span><span className="lp-chip-title">{label}</span></a>
        <Switcher />
        <Menu />
      </NodeViewWrapper>
    );
  }
  return (
    <NodeViewWrapper as="span" className="lp-w" onContextMenu={onCtx}>
      <a href={a.href} onClick={open} className="lp-card">
        {a.image ? <img className="lp-card-img" src={a.image} alt="" /> : null}
        <span className="lp-card-body">
          <span className="lp-card-title">{label}</span>
          {a.description ? <span className="lp-card-desc">{a.description}</span> : null}
          <span className="lp-card-site">{site}</span>
        </span>
      </a>
      <Switcher />
      <Menu />
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
    // renderHTML is overridden below to emit data-* attributes; suppress the
    // auto-rendered plain attrs (display="", title="" …) so the stored HTML
    // stays clean. href is still auto-parsed/rendered via the node's parseHTML.
    const noRender = { renderHTML: () => ({}) };
    return {
      href: { default: '' },
      display: { default: 'card', ...noRender },
      title: { default: '', ...noRender },
      description: { default: '', ...noRender },
      image: { default: '', ...noRender },
      site: { default: '', ...noRender },
    };
  },
  parseHTML() {
    // priority must beat StarterKit's Link mark (1000 at the extension level,
    // but 50 at the ProseMirror rule level since Tiptap does not propagate it).
    // ProseMirror inserts mark rules before node rules at equal priority, so
    // without this an <a data-link-preview> round-tripped through markdown is
    // parsed as a plain link mark and the preview node is lost.
    return [{ tag: 'a[data-link-preview]', priority: 100, getAttrs: el => {
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
