import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import ConfirmDialog from './ConfirmDialog';

describe('ConfirmDialog', () => {
  it('calls onConfirm and onCancel', () => {
    const onConfirm = vi.fn();
    const onCancel = vi.fn();
    render(<ConfirmDialog title="T" message="M" confirmLabel="Los" onConfirm={onConfirm} onCancel={onCancel} />);
    expect(screen.getByText('M')).toBeInTheDocument();
    fireEvent.click(screen.getByText('Los'));
    expect(onConfirm).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByText('Abbrechen'));
    expect(onCancel).toHaveBeenCalledOnce();
  });
});
