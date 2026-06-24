import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
vi.mock('../linkMeta', () => ({ fetchMeta: vi.fn(() => Promise.resolve({ url: '', title: '', description: '', image: '', site: '' })) }));
vi.mock('../api', () => ({ api: { openExternal: vi.fn() } }));
vi.mock('@tiptap/react', () => ({
  NodeViewWrapper: ({ children, onContextMenu }: { children: React.ReactNode; onContextMenu?: (e: unknown) => void }) => <span onContextMenu={onContextMenu}>{children}</span>,
  ReactNodeViewRenderer: () => () => null,
}));
import { LinkPreviewView, LinkPreviewCtx } from './LinkPreviewNode';
import { api } from '../api';

const props = (extra: Record<string, unknown> = {}) => ({
  node: { attrs: { href: 'https://ex.com/a', display: 'card', title: 'Titel', description: '', image: '', site: 'ex.com' } },
  updateAttributes: vi.fn(), deleteNode: vi.fn(), selected: false,
  editor: {} as never, getPos: () => 0, extension: {} as never, decorations: [], view: {} as never, innerDecorations: {} as never, ...extra,
} as never);

describe('LinkPreviewView context menu', () => {
  it('right-click opens the menu; "open" calls openExternal', () => {
    render(<LinkPreviewCtx.Provider value={{ enabled: true, mode: 'card' }}><LinkPreviewView {...props()} /></LinkPreviewCtx.Provider>);
    fireEvent.contextMenu(screen.getByText('Titel'));
    fireEvent.click(screen.getByText('Link öffnen'));
    expect(api.openExternal).toHaveBeenCalledWith('https://ex.com/a');
  });
});
