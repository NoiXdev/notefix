import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import UpdateBanner from './UpdateBanner';
import type { UpdateInfo } from '../api';

const info = (overrides: Partial<UpdateInfo> = {}): UpdateInfo => ({
  current: '0.7.0',
  latest: '0.8.0',
  updateAvailable: true,
  url: 'https://example.com/release',
  ...overrides,
});

describe('UpdateBanner', () => {
  it('shows the banner text with the available version', () => {
    render(<UpdateBanner info={info()} onDownload={vi.fn()} onDismiss={vi.fn()} />);
    expect(screen.getByText('Neue Version 0.8.0 verfügbar.')).toBeInTheDocument();
  });

  it('calls onDownload when the download/install button is clicked', () => {
    const onDownload = vi.fn();
    render(<UpdateBanner info={info()} onDownload={onDownload} onDismiss={vi.fn()} />);
    fireEvent.click(screen.getByText('Herunterladen'));
    expect(onDownload).toHaveBeenCalledOnce();
  });

  it('calls onDismiss when the dismiss button is clicked', () => {
    const onDismiss = vi.fn();
    render(<UpdateBanner info={info()} onDownload={vi.fn()} onDismiss={onDismiss} />);
    fireEvent.click(screen.getByLabelText('Ausblenden'));
    expect(onDismiss).toHaveBeenCalledOnce();
  });

  it('renders a different version string when info changes', () => {
    render(<UpdateBanner info={info({ latest: '1.2.3' })} onDownload={vi.fn()} onDismiss={vi.fn()} />);
    expect(screen.getByText('Neue Version 1.2.3 verfügbar.')).toBeInTheDocument();
  });
});
