import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';

vi.mock('@tiptap/react', () => ({
  NodeViewWrapper: ({ children, className, style }: { children: React.ReactNode; className?: string; style?: React.CSSProperties }) => (
    <div data-testid="wrapper" className={className} style={style}>{children}</div>
  ),
  ReactNodeViewRenderer: () => () => null,
}));

import { ImageNodeView, MIN_WIDTH } from './ResizableImage';
import type { NodeViewProps } from '@tiptap/react';

function makeProps(overrides: Partial<{ attrs: Record<string, unknown>; selected: boolean; isEditable: boolean }> = {}) {
  const { attrs = {}, selected = false, isEditable = true } = overrides;
  const updateAttributes = vi.fn();
  const props = {
    node: { attrs: { src: 'https://ex.com/a.png', alt: null, title: null, width: null, ...attrs } },
    updateAttributes,
    selected,
    editor: { isEditable },
    getPos: () => 0,
    deleteNode: vi.fn(),
    extension: {} as never,
    decorations: [],
    view: {} as never,
    innerDecorations: {} as never,
  } as unknown as NodeViewProps;
  return { props, updateAttributes };
}

describe('ImageNodeView', () => {
  it('renders the image with src, alt and title from node attrs', () => {
    const { props } = makeProps({ attrs: { src: 'https://ex.com/a.png', alt: 'A cat', title: 'Cat picture' } });
    render(<ImageNodeView {...props} />);
    const img = screen.getByRole('img') as HTMLImageElement;
    expect(img.src).toBe('https://ex.com/a.png');
    expect(img.alt).toBe('A cat');
    expect(img.title).toBe('Cat picture');
    expect(img.draggable).toBe(false);
  });

  it('defaults alt/title to empty strings when attrs are null', () => {
    const { props } = makeProps();
    render(<ImageNodeView {...props} />);
    const img = document.querySelector('img') as HTMLImageElement;
    expect(img.alt).toBe('');
    expect(img.title).toBe('');
  });

  it('adds the is-selected class when selected', () => {
    const { props } = makeProps({ selected: true });
    render(<ImageNodeView {...props} />);
    expect(screen.getByTestId('wrapper').className).toContain('is-selected');
  });

  it('omits the is-selected class when not selected', () => {
    const { props } = makeProps({ selected: false });
    render(<ImageNodeView {...props} />);
    expect(screen.getByTestId('wrapper').className).not.toContain('is-selected');
  });

  it('applies a pixel width style for a numeric width attr', () => {
    const { props } = makeProps({ attrs: { width: 240 } });
    render(<ImageNodeView {...props} />);
    expect(screen.getByTestId('wrapper')).toHaveStyle({ width: '240px' });
  });

  it('applies a string width attr as-is', () => {
    const { props } = makeProps({ attrs: { width: '50%' } });
    render(<ImageNodeView {...props} />);
    expect(screen.getByTestId('wrapper')).toHaveStyle({ width: '50%' });
  });

  it('applies no width style when the width attr is null', () => {
    const { props } = makeProps({ attrs: { width: null } });
    render(<ImageNodeView {...props} />);
    expect(screen.getByTestId('wrapper')).not.toHaveStyle({ width: expect.anything() });
  });

  it('shows the resize handle when the editor is editable', () => {
    const { props } = makeProps({ isEditable: true });
    const { container } = render(<ImageNodeView {...props} />);
    expect(container.querySelector('.image-resize-handle')).toBeInTheDocument();
  });

  it('hides the resize handle when the editor is not editable', () => {
    const { props } = makeProps({ isEditable: false });
    const { container } = render(<ImageNodeView {...props} />);
    expect(container.querySelector('.image-resize-handle')).not.toBeInTheDocument();
  });

  it('dragging the handle updates the width attribute by the pointer delta', () => {
    const { props, updateAttributes } = makeProps({ isEditable: true });
    const { container } = render(<ImageNodeView {...props} />);
    const img = container.querySelector('img')!;
    vi.spyOn(img, 'getBoundingClientRect').mockReturnValue({ width: 200 } as DOMRect);
    const handle = container.querySelector('.image-resize-handle')!;

    fireEvent.pointerDown(handle, { clientX: 100 });
    fireEvent.pointerMove(window, { clientX: 130 });
    expect(updateAttributes).toHaveBeenCalledWith({ width: 230 });

    fireEvent.pointerMove(window, { clientX: 150 });
    expect(updateAttributes).toHaveBeenCalledWith({ width: 250 });
  });

  it('clamps the resized width to MIN_WIDTH when dragging far left', () => {
    const { props, updateAttributes } = makeProps({ isEditable: true });
    const { container } = render(<ImageNodeView {...props} />);
    const img = container.querySelector('img')!;
    vi.spyOn(img, 'getBoundingClientRect').mockReturnValue({ width: 200 } as DOMRect);
    const handle = container.querySelector('.image-resize-handle')!;

    fireEvent.pointerDown(handle, { clientX: 100 });
    fireEvent.pointerMove(window, { clientX: -1000 });
    expect(updateAttributes).toHaveBeenCalledWith({ width: MIN_WIDTH });
  });

  it('stops updating after pointerup', () => {
    const { props, updateAttributes } = makeProps({ isEditable: true });
    const { container } = render(<ImageNodeView {...props} />);
    const img = container.querySelector('img')!;
    vi.spyOn(img, 'getBoundingClientRect').mockReturnValue({ width: 200 } as DOMRect);
    const handle = container.querySelector('.image-resize-handle')!;

    fireEvent.pointerDown(handle, { clientX: 100 });
    fireEvent.pointerMove(window, { clientX: 130 });
    expect(updateAttributes).toHaveBeenCalledTimes(1);
    fireEvent.pointerUp(window, { clientX: 130 });
    fireEvent.pointerMove(window, { clientX: 999 });
    expect(updateAttributes).toHaveBeenCalledTimes(1);
  });
});
