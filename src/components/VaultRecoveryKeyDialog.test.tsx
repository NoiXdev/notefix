import { render, screen, fireEvent } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import VaultRecoveryKeyDialog from './VaultRecoveryKeyDialog';

describe('VaultRecoveryKeyDialog', () => {
  it('shows the key once and closes only after the user confirms they saved it', () => {
    const onClose = vi.fn();
    render(<VaultRecoveryKeyDialog groups={['AAAA', 'BBBB', 'CCCC']} onClose={onClose} />);
    expect(screen.getByText('AAAA-BBBB-CCCC')).toBeInTheDocument();
    const done = screen.getByRole('button', { name: 'Ich habe den Schlüssel gesichert' });
    expect(done).toBeDisabled();
    fireEvent.click(screen.getByLabelText('Ich habe den Schlüssel an einem sicheren Ort gespeichert'));
    expect(done).toBeEnabled();
    fireEvent.click(done);
    expect(onClose).toHaveBeenCalledOnce();
  });
});
