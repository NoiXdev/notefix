import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import ExportDialog from './ExportDialog';

describe('ExportDialog', () => {
  it('calls the right callback per option', () => {
    const onBase64 = vi.fn(), onBundle = vi.fn(), onCancel = vi.fn();
    render(<ExportDialog onBase64={onBase64} onBundle={onBundle} onCancel={onCancel} />);
    fireEvent.click(screen.getByText(/base64/));
    expect(onBase64).toHaveBeenCalled();
    fireEvent.click(screen.getByText(/Bundle/));
    expect(onBundle).toHaveBeenCalled();
  });
});
