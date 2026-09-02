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

  it('does not call setup while either field is empty, and the create button is disabled', () => {
    const { setupFn } = renderSetup();
    const create = screen.getByText('Einrichten');
    expect(create).toBeDisabled();
    fireEvent.click(create);
    expect(setupFn).not.toHaveBeenCalled();
  });

  it('shows the setup error message and stays on the form when setup rejects', async () => {
    const setupFn = vi.fn<(passphrase: string) => Promise<string[]>>().mockRejectedValue(new Error('boom'));
    renderSetup(setupFn);
    fireEvent.change(screen.getByPlaceholderText('Passwort'), { target: { value: 'abc123' } });
    fireEvent.change(screen.getByPlaceholderText('Passwort bestätigen'), { target: { value: 'abc123' } });
    fireEvent.click(screen.getByText('Einrichten'));
    expect(await screen.findByRole('alert')).toHaveTextContent('boom');
    expect(screen.getByText('Tresor einrichten')).toBeInTheDocument();
  });

  it('submits on Enter in either field', () => {
    const { setupFn } = renderSetup();
    const passphrase = screen.getByPlaceholderText('Passwort');
    const confirm = screen.getByPlaceholderText('Passwort bestätigen');
    fireEvent.change(passphrase, { target: { value: 'abc123' } });
    fireEvent.change(confirm, { target: { value: 'abc123' } });
    fireEvent.keyDown(confirm, { key: 'Enter' });
    expect(setupFn).toHaveBeenCalledWith('abc123');
  });

  it('calls onCancel on Escape, on the cancel button, and on a backdrop click', () => {
    const { onCancel } = renderSetup();
    fireEvent.keyDown(screen.getByPlaceholderText('Passwort'), { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByText('Abbrechen'));
    expect(onCancel).toHaveBeenCalledTimes(2);

    fireEvent.click(screen.getByText('Tresor einrichten').closest('div.fixed')!);
    expect(onCancel).toHaveBeenCalledTimes(3);
  });

  it('does not call onCancel when clicking inside the dialog panel', () => {
    const { onCancel } = renderSetup();
    fireEvent.click(screen.getByText('Tresor einrichten'));
    expect(onCancel).not.toHaveBeenCalled();
  });

  it('copies the recovery key to the clipboard and shows "copied" temporarily', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });
    const setupFn = vi.fn<(passphrase: string) => Promise<string[]>>().mockResolvedValue(['ABCDE', 'FGHIJ']);
    renderSetup(setupFn);
    fireEvent.change(screen.getByPlaceholderText('Passwort'), { target: { value: 'abc123' } });
    fireEvent.change(screen.getByPlaceholderText('Passwort bestätigen'), { target: { value: 'abc123' } });
    fireEvent.click(screen.getByText('Einrichten'));

    await waitFor(() => expect(screen.getByText('ABCDE-FGHIJ')).toBeInTheDocument());
    fireEvent.click(screen.getByText('Kopieren'));
    expect(writeText).toHaveBeenCalledWith('ABCDE-FGHIJ');
    expect(await screen.findByText('Kopiert')).toBeInTheDocument();

    await waitFor(() => expect(screen.getByText('Kopieren')).toBeInTheDocument(), { timeout: 3000 });
  }, 10000);
});
