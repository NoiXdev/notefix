import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import FolderCustomizer from './FolderCustomizer';
import type { Folder } from '../types';

vi.mock('emoji-picker-react', () => ({
  default: ({ onEmojiClick }: { onEmojiClick: (d: { emoji: string }) => void }) => (
    <button onClick={() => onEmojiClick({ emoji: '🎉' })}>emoji-mock</button>
  ),
  Theme: { DARK: 'dark' },
}));

const folder: Folder = { id: 'f1', name: 'A', parentId: null, position: 0, icon: '', color: '', sort: 'manual' };

describe('FolderCustomizer', () => {
  it('FA mode shows the icon search and picks fa:star', () => {
    const onSetIcon = vi.fn();
    render(<FolderCustomizer x={0} y={0} folder={folder} onSetIcon={onSetIcon} onSetColor={vi.fn()} onClose={vi.fn()} />);
    fireEvent.click(screen.getByText('Font Awesome'));
    fireEvent.change(screen.getByPlaceholderText('Icon suchen…'), { target: { value: 'star' } });
    fireEvent.click(screen.getByText('star'));
    expect(onSetIcon).toHaveBeenCalledWith('fa:star');
  });
  it('Emoji mode uses the emoji picker', () => {
    const onSetIcon = vi.fn();
    render(<FolderCustomizer x={0} y={0} folder={folder} onSetIcon={onSetIcon} onSetColor={vi.fn()} onClose={vi.fn()} />);
    fireEvent.click(screen.getByText('Emoji'));
    fireEvent.click(screen.getByText('emoji-mock'));
    expect(onSetIcon).toHaveBeenCalledWith('🎉');
  });
  it('Standard mode clears the icon', () => {
    const onSetIcon = vi.fn();
    render(<FolderCustomizer x={0} y={0} folder={{ ...folder, icon: 'fa:star' }} onSetIcon={onSetIcon} onSetColor={vi.fn()} onClose={vi.fn()} />);
    fireEvent.click(screen.getByText('Standard'));
    expect(onSetIcon).toHaveBeenCalledWith('');
  });
  it('a color swatch sets the color', () => {
    const onSetColor = vi.fn();
    render(<FolderCustomizer x={0} y={0} folder={folder} onSetIcon={vi.fn()} onSetColor={onSetColor} onClose={vi.fn()} />);
    fireEvent.click(screen.getByLabelText('Farbe #22c55e'));
    expect(onSetColor).toHaveBeenCalledWith('#22c55e');
  });
});
