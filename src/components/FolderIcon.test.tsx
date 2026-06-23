import { render } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import FolderIcon from './FolderIcon';

describe('FolderIcon', () => {
  it('renders a FA icon for fa:star', () => {
    const { container } = render(<FolderIcon icon="fa:star" />);
    expect(container.querySelector('[data-icon="star"]')).toBeTruthy();
  });
  it('renders an emoji as text', () => {
    const { getByText } = render(<FolderIcon icon="📁" />);
    expect(getByText('📁')).toBeInTheDocument();
  });
  it('renders the default folder svg when empty and tints it', () => {
    const { container } = render(<FolderIcon icon="" tint="#22c55e" />);
    const svg = container.querySelector('svg');
    expect(svg).toBeTruthy();
    expect(svg?.getAttribute('stroke')).toBe('#22c55e');
  });
});
