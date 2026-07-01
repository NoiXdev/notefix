import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import type { Editor } from '@tiptap/react';
import FindBar from './FindBar';

const { searchState } = vi.hoisted(() => ({
  searchState: vi.fn(() => ({ matches: [1, 2, 3], current: 0 })),
}));
vi.mock('../editor/searchHighlight', () => ({ searchState }));

function fakeEditor() {
  return {
    isDestroyed: false,
    commands: { setSearch: vi.fn(), stepSearch: vi.fn(), clearSearch: vi.fn() },
  } as unknown as Editor & { commands: Record<string, ReturnType<typeof vi.fn>> };
}

describe('FindBar', () => {
  it('runs the search and shows the match counter while typing', () => {
    const editor = fakeEditor();
    render(<FindBar editor={editor} onClose={() => {}} />);
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'foo' } });
    expect(editor.commands.setSearch).toHaveBeenCalledWith('foo');
    expect(screen.getByText('1/3')).toBeInTheDocument();
  });

  it('steps forward on Enter and backward on Shift+Enter', () => {
    const editor = fakeEditor();
    render(<FindBar editor={editor} onClose={() => {}} />);
    const input = screen.getByRole('textbox');
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(editor.commands.stepSearch).toHaveBeenCalledWith(1);
    fireEvent.keyDown(input, { key: 'Enter', shiftKey: true });
    expect(editor.commands.stepSearch).toHaveBeenCalledWith(-1);
  });

  it('closes on Escape', () => {
    const onClose = vi.fn();
    render(<FindBar editor={fakeEditor()} onClose={onClose} />);
    fireEvent.keyDown(screen.getByRole('textbox'), { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });

  it('closes when clicking outside the bar', () => {
    const onClose = vi.fn();
    render(<FindBar editor={fakeEditor()} onClose={onClose} />);
    fireEvent.mouseDown(document.body);
    expect(onClose).toHaveBeenCalled();
  });

  it('does not close when clicking inside the bar', () => {
    const onClose = vi.fn();
    render(<FindBar editor={fakeEditor()} onClose={onClose} />);
    fireEvent.mouseDown(screen.getByRole('textbox'));
    expect(onClose).not.toHaveBeenCalled();
  });

  it('ignores clicks on the toolbar find toggle so its own handler can close', () => {
    const onClose = vi.fn();
    const toggle = document.createElement('button');
    toggle.setAttribute('data-find-toggle', '');
    document.body.appendChild(toggle);
    render(<FindBar editor={fakeEditor()} onClose={onClose} />);
    fireEvent.mouseDown(toggle);
    expect(onClose).not.toHaveBeenCalled();
    toggle.remove();
  });

  it('clears the highlight when it unmounts', () => {
    const editor = fakeEditor();
    const { unmount } = render(<FindBar editor={editor} onClose={() => {}} />);
    unmount();
    expect(editor.commands.clearSearch).toHaveBeenCalled();
  });
});
