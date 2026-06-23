import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import CloseDialog from './CloseDialog';
describe('CloseDialog', () => {
  it('quit/minimize pass the remember flag, cancel works', () => {
    const onMinimize = vi.fn(); const onQuit = vi.fn(); const onCancel = vi.fn();
    render(<CloseDialog onMinimize={onMinimize} onQuit={onQuit} onCancel={onCancel} />);
    fireEvent.click(screen.getByLabelText(/merken/i));
    fireEvent.click(screen.getByText('Beenden'));
    expect(onQuit).toHaveBeenCalledWith(true);
    fireEvent.click(screen.getByText('In Menüleiste'));
    expect(onMinimize).toHaveBeenCalledWith(true);
    fireEvent.click(screen.getByText('Abbrechen'));
    expect(onCancel).toHaveBeenCalled();
  });
});
