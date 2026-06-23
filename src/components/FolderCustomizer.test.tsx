import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import FolderCustomizer from './FolderCustomizer';
import type { Folder } from '../types';

const folder: Folder = { id: 'f1', name: 'A', parentId: null, position: 0, icon: '', color: '' };

describe('FolderCustomizer', () => {
  it('searching then clicking an icon calls onSetIcon with fa:name', () => {
    const onSetIcon = vi.fn();
    render(<FolderCustomizer x={0} y={0} folder={folder} onSetIcon={onSetIcon} onSetColor={vi.fn()} onClose={vi.fn()} />);
    fireEvent.change(screen.getByPlaceholderText('Icon suchen…'), { target: { value: 'star' } });
    fireEvent.click(screen.getByLabelText('star'));
    expect(onSetIcon).toHaveBeenCalledWith('fa:star');
  });
  it('clicking an emoji calls onSetIcon with the emoji', () => {
    const onSetIcon = vi.fn();
    render(<FolderCustomizer x={0} y={0} folder={folder} onSetIcon={onSetIcon} onSetColor={vi.fn()} onClose={vi.fn()} />);
    fireEvent.click(screen.getByText('📁'));
    expect(onSetIcon).toHaveBeenCalledWith('📁');
  });
  it('Standard clears the icon and a swatch sets the color', () => {
    const onSetIcon = vi.fn();
    const onSetColor = vi.fn();
    render(<FolderCustomizer x={0} y={0} folder={folder} onSetIcon={onSetIcon} onSetColor={onSetColor} onClose={vi.fn()} />);
    fireEvent.click(screen.getByLabelText('Standard'));
    expect(onSetIcon).toHaveBeenCalledWith('');
    fireEvent.click(screen.getByLabelText('Farbe #22c55e'));
    expect(onSetColor).toHaveBeenCalledWith('#22c55e');
  });
});
