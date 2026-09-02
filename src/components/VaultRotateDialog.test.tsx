import { render, screen, fireEvent, waitFor, cleanup } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import VaultRotateDialog from './VaultRotateDialog';
import type { RotationCode } from '../types';

function renderDialog(overrides: Partial<Parameters<typeof VaultRotateDialog>[0]> = {}) {
  const rotate = vi
    .fn<(passphrase: string, recoveryKey?: string) => Promise<RotationCode[]>>()
    .mockResolvedValue([{ userId: 2, code: 'AAAAA-BBBBB' }]);
  const onSuccess = vi.fn();
  const onCancel = vi.fn();
  render(
    <VaultRotateDialog
      recoveryHolder={false}
      rotate={rotate}
      onSuccess={onSuccess}
      onCancel={onCancel}
      {...overrides}
    />,
  );
  return { rotate, onSuccess, onCancel };
}

describe('VaultRotateDialog', () => {
  it('rotates with the passphrase and hands the codes on', async () => {
    const { rotate, onSuccess } = renderDialog();
    fireEvent.change(screen.getByPlaceholderText('Passwort'), { target: { value: 'owner-pw' } });
    fireEvent.click(screen.getByRole('button', { name: 'Schlüssel wechseln' }));

    await waitFor(() => expect(rotate).toHaveBeenCalledWith('owner-pw', undefined));
    await waitFor(() => expect(onSuccess).toHaveBeenCalledWith([{ userId: 2, code: 'AAAAA-BBBBB' }]));
  });

  it('asks a recovery-key holder for the key and submits it', async () => {
    const { rotate } = renderDialog({ recoveryHolder: true });
    fireEvent.change(screen.getByPlaceholderText('Passwort'), { target: { value: 'owner-pw' } });
    const keyField = screen.getByPlaceholderText('Wiederherstellungs-Schlüssel');
    fireEvent.change(keyField, { target: { value: 'AAAAA-BBBBB-CCCCC' } });
    fireEvent.keyDown(keyField, { key: 'Enter' });

    await waitFor(() => expect(rotate).toHaveBeenCalledWith('owner-pw', 'AAAAA-BBBBB-CCCCC'));
  });

  it('keeps the button disabled until a holder typed both secrets', () => {
    renderDialog({ recoveryHolder: true });
    const submit = screen.getByRole('button', { name: 'Schlüssel wechseln' });
    expect(submit).toBeDisabled();
    fireEvent.change(screen.getByPlaceholderText('Passwort'), { target: { value: 'owner-pw' } });
    expect(submit).toBeDisabled();
    fireEvent.change(screen.getByPlaceholderText('Wiederherstellungs-Schlüssel'), { target: { value: 'k' } });
    expect(submit).toBeEnabled();
  });

  it('reports every backend refusal in the user\u2019s own language', async () => {
    const cases: [string, string][] = [
      ['wrong passphrase', 'Falsches Passwort'],
      ['wrong recovery key', 'Falscher Wiederherstellungs-Schlüssel'],
      ['no rotation pending', 'Kein Schlüsselwechsel offen'],
      ['vault locked', 'Entsperre zuerst den Tresor.'],
      ['vault rotate HTTP 403', 'Nur der Besitzer des Arbeitsbereichs kann den Schlüssel wechseln.'],
      // Only a genuinely unforeseen failure falls through to the raw text.
      ['members HTTP 500', 'Schlüsselwechsel fehlgeschlagen: members HTTP 500'],
    ];
    for (const [backendError, shown] of cases) {
      const { rotate, onSuccess } = renderDialog();
      rotate.mockRejectedValueOnce(new Error(backendError));
      fireEvent.change(screen.getByPlaceholderText('Passwort'), { target: { value: 'pw' } });
      fireEvent.click(screen.getByRole('button', { name: 'Schlüssel wechseln' }));

      expect(await screen.findByRole('alert')).toHaveTextContent(shown);
      expect(onSuccess).not.toHaveBeenCalled();
      // The dialog stays open, so the next case starts from a clean DOM.
      cleanup();
    }
  });

  it('cancels on Escape', () => {
    const { onCancel, rotate } = renderDialog();
    fireEvent.keyDown(screen.getByPlaceholderText('Passwort'), { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledOnce();
    expect(rotate).not.toHaveBeenCalled();
  });
});
