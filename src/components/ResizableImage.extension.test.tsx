import { describe, it, expect, vi } from 'vitest';

vi.mock('@tiptap/react', () => ({
  NodeViewWrapper: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
  ReactNodeViewRenderer: vi.fn((component: unknown) => component),
}));

import { ResizableImage } from './ResizableImage';
import { ReactNodeViewRenderer } from '@tiptap/react';

// ResizableImage is a Tiptap `Image.extend({...})` config; `.config` holds the
// override functions verbatim, so the width attribute's parse/render logic
// can be exercised directly without a live ProseMirror EditorView.
type WidthAttr = { default: unknown; parseHTML: (el: Element) => unknown; renderHTML: (attrs: { width: unknown }) => Record<string, string> };
type NodeConfig = { addAttributes: () => { width: WidthAttr }; addNodeView: () => unknown };
const config = (ResizableImage as unknown as { config: NodeConfig }).config;
const widthAttr = config.addAttributes.call({}).width;

describe('ResizableImage node config — width attribute', () => {
  it('defaults to null', () => {
    expect(widthAttr.default).toBeNull();
  });

  it('parses a numeric width attribute off the element', () => {
    const el = document.createElement('img');
    el.setAttribute('width', '320');
    expect(widthAttr.parseHTML(el)).toBe(320);
  });

  it('keeps a non-numeric width attribute as a string', () => {
    const el = document.createElement('img');
    el.setAttribute('width', '50%');
    expect(widthAttr.parseHTML(el)).toBe('50%');
  });

  it('falls back to a numeric inline style width when the attribute is absent', () => {
    const el = document.createElement('img');
    el.style.width = '150px';
    expect(widthAttr.parseHTML(el)).toBe(150);
  });

  it('parses a leading-digit inline style width (parseInt semantics) to a number', () => {
    const el = document.createElement('img');
    el.style.width = '50%';
    expect(widthAttr.parseHTML(el)).toBe(50);
  });

  it('keeps a non-numeric inline style width as a string', () => {
    const el = document.createElement('img');
    el.style.width = 'auto';
    expect(widthAttr.parseHTML(el)).toBe('auto');
  });

  it('returns null when there is neither a width attribute nor a style', () => {
    const el = document.createElement('img');
    expect(widthAttr.parseHTML(el)).toBeNull();
  });

  it('renders an empty object when the width attr is null', () => {
    expect(widthAttr.renderHTML({ width: null })).toEqual({});
  });

  it('renders the width as a string HTML attribute otherwise', () => {
    expect(widthAttr.renderHTML({ width: 320 })).toEqual({ width: '320' });
  });
});

describe('ResizableImage node config — addNodeView', () => {
  it('wires the node view through ReactNodeViewRenderer', () => {
    config.addNodeView();
    expect(ReactNodeViewRenderer).toHaveBeenCalled();
  });
});
