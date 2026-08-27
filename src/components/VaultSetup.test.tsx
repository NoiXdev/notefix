import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import VaultSetup from './VaultSetup';

function renderSetup(setupFn = vi.fn<(passphrase: string) => Promise<string[]>>()) {
  const onSuccess = vi.fn();
  const onCancel = vi.fn();
  render(<VaultSetup setup={setupFn} onSuccess={onSuccess} onCancel={onCancel} />);
  return { setupFn, onSuccess, onCancel };
}

describe('VaultSetup', () => {
  it('renders the setup form', () => {
    renderSetup();
    expect(screen.getByText('Tresor einrichten')).toBeInTheDocument();
  });

  it('shows a mismatch error and does not call setup when passphrases differ', () => {
    const { setupFn } = renderSetup();
    fireEvent.change(screen.getByPlaceholderText('Passwort'), { target: { value: 'abc123' } });
    fireEvent.change(screen.getByPlaceholderText('Passwort bestätigen'), { target: { value: 'xyz789' } });
    fireEvent.click(screen.getByText('Einrichten'));
    expect(screen.getByText('Passwörter stimmen nicht überein')).toBeInTheDocument();
    expect(setupFn).not.toHaveBeenCalled();
  });

  it('calls setup and renders the recovery groups once it resolves', async () => {
    const setupFn = vi.fn<(passphrase: string) => Promise<string[]>>().mockResolvedValue(['ABCDE', 'FGHIJ']);
    const { onSuccess } = renderSetup(setupFn);
    fireEvent.change(screen.getByPlaceholderText('Passwort'), { target: { value: 'abc123' } });
    fireEvent.change(screen.getByPlaceholderText('Passwort bestätigen'), { target: { value: 'abc123' } });
    fireEvent.click(screen.getByText('Einrichten'));
    expect(setupFn).toHaveBeenCalledWith('abc123');

    await waitFor(() => expect(screen.getByText('ABCDE-FGHIJ')).toBeInTheDocument());
    expect(onSuccess).not.toHaveBeenCalled();

    fireEvent.click(screen.getByText('Ich habe ihn gespeichert'));
    expect(onSuccess).toHaveBeenCalledOnce();
  });
});
