import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import PromptDialog from './PromptDialog';

function setup(initialValue?: string) {
  const onSubmit = vi.fn();
  const onCancel = vi.fn();
  render(<PromptDialog title="Titel" confirmLabel="OK" initialValue={initialValue} onSubmit={onSubmit} onCancel={onCancel} />);
  return { onSubmit, onCancel, input: screen.getByRole('textbox') as HTMLInputElement };
}

describe('PromptDialog', () => {
  it('submits the trimmed typed value via the confirm button', () => {
    const { onSubmit, input } = setup();
    fireEvent.change(input, { target: { value: '  Arbeit  ' } });
    fireEvent.click(screen.getByText('OK'));
    expect(onSubmit).toHaveBeenCalledWith('Arbeit');
  });

  it('submits on Enter', () => {
    const { onSubmit, input } = setup('Privat');
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(onSubmit).toHaveBeenCalledWith('Privat');
  });

  it('does not submit an empty/whitespace value', () => {
    const { onSubmit, input } = setup();
    fireEvent.change(input, { target: { value: '   ' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it('cancels on Escape and on the cancel button', () => {
    const { onCancel, input } = setup();
    fireEvent.keyDown(input, { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledTimes(1);
  });
});
