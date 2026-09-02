import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';

const { mockFetchMeta } = vi.hoisted(() => ({
  mockFetchMeta: vi.fn(() => Promise.resolve({ url: '', title: '', description: '', image: '', site: '' })),
}));
vi.mock('../linkMeta', () => ({ fetchMeta: mockFetchMeta }));
vi.mock('../api', () => ({ api: { openExternal: vi.fn() } }));
import { LinkPreviewView, LinkPreviewCtx } from './LinkPreviewNode';
import { api } from '../api';

vi.mock('@tiptap/react', () => ({
  NodeViewWrapper: ({ children, as: As = 'span', onContextMenu }: { children: React.ReactNode; as?: string; onContextMenu?: (e: unknown) => void }) => {
    const Tag = As as 'span';
    return <Tag onContextMenu={onContextMenu}>{children}</Tag>;
  },
  ReactNodeViewRenderer: () => () => null,
}));

const props = (extra: Record<string, unknown> = {}, display = 'card') => ({
  node: { attrs: { href: 'https://ex.com/a', display, title: 'Titel', description: 'Desc', image: '', site: 'ex.com', ...extra } },
  updateAttributes: vi.fn(), deleteNode: vi.fn(), selected: false,
  editor: {} as never, getPos: () => 0, extension: {} as never, decorations: [], view: {} as never, innerDecorations: {} as never,
} as never);

beforeEach(() => {
  vi.clearAllMocks();
  mockFetchMeta.mockResolvedValue({ url: '', title: '', description: '', image: '', site: '' });
});

describe('LinkPreviewView', () => {
  it('renders the card title + site in card mode', () => {
    render(<LinkPreviewCtx.Provider value={{ enabled: true, mode: 'card' }}><LinkPreviewView {...props()} /></LinkPreviewCtx.Provider>);
    expect(screen.getByText('Titel')).toBeInTheDocument();
    expect(screen.getByText('ex.com')).toBeInTheDocument();
  });
  it('falls back to a plain link when disabled', () => {
    render(<LinkPreviewCtx.Provider value={{ enabled: false, mode: 'card' }}><LinkPreviewView {...props()} /></LinkPreviewCtx.Provider>);
    const link = screen.getByText('Titel');
    expect(link.closest('a')?.className).toContain('lp-url');
  });

  it('renders a plain link with the label text in url mode', () => {
    render(<LinkPreviewCtx.Provider value={{ enabled: true, mode: 'url' }}><LinkPreviewView {...props({}, 'url')} /></LinkPreviewCtx.Provider>);
    const link = screen.getByText('Titel').closest('a');
    expect(link?.className).toBe('lp-url');
    expect(link).toHaveAttribute('href', 'https://ex.com/a');
  });

  it('falls back to the href as the label when there is no title', () => {
    render(<LinkPreviewCtx.Provider value={{ enabled: true, mode: 'url' }}><LinkPreviewView {...props({ title: '' }, 'url')} /></LinkPreviewCtx.Provider>);
    expect(screen.getByText('https://ex.com/a')).toBeInTheDocument();
  });

  it('renders a chip with the site and title in inline mode', () => {
    render(<LinkPreviewCtx.Provider value={{ enabled: true, mode: 'inline' }}><LinkPreviewView {...props({}, 'inline')} /></LinkPreviewCtx.Provider>);
    const link = screen.getByTitle('Titel');
    expect(link.className).toBe('lp-chip');
    expect(link.querySelector('.lp-chip-site')).toHaveTextContent('ex.com');
    expect(link.querySelector('.lp-chip-title')).toHaveTextContent('Titel');
  });

  it('falls back to the parsed hostname when site is empty', () => {
    render(<LinkPreviewCtx.Provider value={{ enabled: true, mode: 'inline' }}><LinkPreviewView {...props({ site: '', href: 'https://www.example.org/path' }, 'inline')} /></LinkPreviewCtx.Provider>);
    expect(screen.getByText('example.org')).toBeInTheDocument();
  });

  it('domainOf falls back to the raw href for an unparseable URL', () => {
    render(<LinkPreviewCtx.Provider value={{ enabled: true, mode: 'inline' }}><LinkPreviewView {...props({ site: '', href: 'not-a-url' }, 'inline')} /></LinkPreviewCtx.Provider>);
    expect(screen.getByText('not-a-url')).toBeInTheDocument();
  });

  it('renders the preview image and description in card mode when present', () => {
    render(<LinkPreviewCtx.Provider value={{ enabled: true, mode: 'card' }}><LinkPreviewView {...props({ image: 'https://ex.com/i.png', description: 'Eine Beschreibung' })} /></LinkPreviewCtx.Provider>);
    const img = document.querySelector('img.lp-card-img') as HTMLImageElement;
    expect(img.src).toBe('https://ex.com/i.png');
    expect(screen.getByText('Eine Beschreibung')).toBeInTheDocument();
  });

  it('omits the image and description in card mode when absent', () => {
    render(<LinkPreviewCtx.Provider value={{ enabled: true, mode: 'card' }}><LinkPreviewView {...props({ image: '', description: '' })} /></LinkPreviewCtx.Provider>);
    expect(document.querySelector('img.lp-card-img')).not.toBeInTheDocument();
  });

  it('opens the link via api.openExternal and prevents the default navigation on click', () => {
    render(<LinkPreviewCtx.Provider value={{ enabled: true, mode: 'card' }}><LinkPreviewView {...props()} /></LinkPreviewCtx.Provider>);
    fireEvent.click(screen.getByText('Titel').closest('a')!);
    expect(api.openExternal).toHaveBeenCalledWith('https://ex.com/a');
  });

  it('shows the display switcher when selected and enabled, and switching calls updateAttributes', () => {
    const p = props({}, 'card');
    (p as { selected: boolean }).selected = true;
    render(<LinkPreviewCtx.Provider value={{ enabled: true, mode: 'card' }}><LinkPreviewView {...p} /></LinkPreviewCtx.Provider>);
    const urlBtn = screen.getByText('url');
    expect(screen.getByText('card').className).toContain('on');
    fireEvent.mouseDown(urlBtn);
    expect((p as { updateAttributes: (a: unknown) => void }).updateAttributes).toHaveBeenCalledWith({ display: 'url' });
  });

  it('hides the display switcher when not selected', () => {
    render(<LinkPreviewCtx.Provider value={{ enabled: true, mode: 'card' }}><LinkPreviewView {...props()} /></LinkPreviewCtx.Provider>);
    expect(screen.queryByText('url')).not.toBeInTheDocument();
  });

  it('hides the display switcher when the preview is disabled, even if selected', () => {
    const p = props({}, 'card');
    (p as { selected: boolean }).selected = true;
    render(<LinkPreviewCtx.Provider value={{ enabled: false, mode: 'card' }}><LinkPreviewView {...p} /></LinkPreviewCtx.Provider>);
    expect(screen.queryByText('url')).not.toBeInTheDocument();
  });

  it('fetches meta and applies it to the node when the title is missing', async () => {
    mockFetchMeta.mockResolvedValue({ url: '', title: 'Fetched', description: 'Fetched desc', image: 'https://ex.com/f.png', site: 'ex.com' });
    const p = props({ title: '' });
    render(<LinkPreviewCtx.Provider value={{ enabled: true, mode: 'card' }}><LinkPreviewView {...p} /></LinkPreviewCtx.Provider>);
    await waitFor(() => expect(mockFetchMeta).toHaveBeenCalledWith('https://ex.com/a'));
    await waitFor(() => expect((p as { updateAttributes: ReturnType<typeof vi.fn> }).updateAttributes).toHaveBeenCalledWith({
      title: 'Fetched', description: 'Fetched desc', image: 'https://ex.com/f.png', site: 'ex.com',
    }));
  });

  it('does not fetch meta when a title is already present', () => {
    render(<LinkPreviewCtx.Provider value={{ enabled: true, mode: 'card' }}><LinkPreviewView {...props({ title: 'Already there' })} /></LinkPreviewCtx.Provider>);
    expect(mockFetchMeta).not.toHaveBeenCalled();
  });

  it('does not fetch meta when there is no href', () => {
    render(<LinkPreviewCtx.Provider value={{ enabled: true, mode: 'card' }}><LinkPreviewView {...props({ title: '', href: '' })} /></LinkPreviewCtx.Provider>);
    expect(mockFetchMeta).not.toHaveBeenCalled();
  });
});
