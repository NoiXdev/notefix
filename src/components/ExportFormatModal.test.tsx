import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import ExportFormatModal from './ExportFormatModal';

describe('ExportFormatModal', () => {
  it('renders the five formats and reports the chosen one', () => {
    const onExport = vi.fn();
    render(<ExportFormatModal onExport={onExport} onCancel={vi.fn()} />);
    expect(screen.getByText('Markdown')).toBeInTheDocument();
    expect(screen.getByText('PDF')).toBeInTheDocument();
    expect(screen.getByText('Word')).toBeInTheDocument();
    fireEvent.click(screen.getByText('PDF'));
    expect(onExport).toHaveBeenCalledWith('pdf', false);
  });
  it('passes the markdown-bundle flag', () => {
    const onExport = vi.fn();
    render(<ExportFormatModal onExport={onExport} onCancel={vi.fn()} />);
    fireEvent.click(screen.getByRole('checkbox'));
    fireEvent.click(screen.getByText('Markdown'));
    expect(onExport).toHaveBeenCalledWith('md', true);
  });
});
