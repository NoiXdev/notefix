import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
vi.mock('../linkMeta', () => ({ fetchMeta: vi.fn(() => Promise.resolve({ url: '', title: '', description: '', image: '', site: '' })) }));
vi.mock('../api', () => ({ api: { openExternal: vi.fn() } }));
import { LinkPreviewView, LinkPreviewCtx } from './LinkPreviewNode';

vi.mock('@tiptap/react', () => ({
  NodeViewWrapper: ({ children }: { children: React.ReactNode }) => <span>{children}</span>,
  ReactNodeViewRenderer: () => () => null,
}));

const props = (display: string, enabled = true) => ({
  node: { attrs: { href: 'https://ex.com/a', display, title: 'Titel', description: 'Desc', image: '', site: 'ex.com' } },
  updateAttributes: vi.fn(), selected: false, editor: {} as never, getPos: () => 0, deleteNode: vi.fn(), extension: {} as never, decorations: [], view: {} as never, innerDecorations: {} as never,
} as never);

describe('LinkPreviewView', () => {
  it('renders the card title + site in card mode', () => {
    render(<LinkPreviewCtx.Provider value={{ enabled: true, mode: 'card' }}><LinkPreviewView {...props('card')} /></LinkPreviewCtx.Provider>);
    expect(screen.getByText('Titel')).toBeInTheDocument();
    expect(screen.getByText('ex.com')).toBeInTheDocument();
  });
  it('falls back to a plain link when disabled', () => {
    render(<LinkPreviewCtx.Provider value={{ enabled: false, mode: 'card' }}><LinkPreviewView {...props('card', false)} /></LinkPreviewCtx.Provider>);
    const link = screen.getByText('Titel');
    expect(link.closest('a')?.className).toContain('lp-url');
  });
});
