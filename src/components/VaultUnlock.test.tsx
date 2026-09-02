import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import VaultUnlock from './VaultUnlock';

function renderUnlock(overrides: Partial<Parameters<typeof VaultUnlock>[0]> = {}) {
  const unlock = vi.fn<(passphrase: string) => Promise<void>>().mockResolvedValue(undefined);
  const unlockRecovery = vi.fn<(recovery: string) => Promise<void>>().mockResolvedValue(undefined);
  const unlockBiometric = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
  const onSuccess = vi.fn();
  const onCancel = vi.fn();
  render(
    <VaultUnlock
      biometricAvailable={false}
      unlock={unlock}
      unlockRecovery={unlockRecovery}
      unlockBiometric={unlockBiometric}
      onSuccess={onSuccess}
      onCancel={onCancel}
      {...overrides}
    />,
  );
  return { unlock, unlockRecovery, unlockBiometric, onSuccess, onCancel };
}

describe('VaultUnlock', () => {
  it('renders the unlock title', () => {
    renderUnlock();
    expect(screen.getByText('Tresor entsperren')).toBeInTheDocument();
  });

  it('calls unlock with the typed passphrase and onSuccess on resolve', async () => {
    const { unlock, onSuccess } = renderUnlock();
    fireEvent.change(screen.getByPlaceholderText('Passwort'), { target: { value: 'my-pass' } });
    fireEvent.click(screen.getByText('Entsperren'));
    expect(unlock).toHaveBeenCalledWith('my-pass');
    await waitFor(() => expect(onSuccess).toHaveBeenCalledOnce());
  });

  it('shows a Touch ID button that calls unlockBiometric when clicked again', async () => {
    const { unlockBiometric, onSuccess } = renderUnlock({ biometricAvailable: true });
    // The mount-time auto-trigger (below) already fires it once and succeeds.
    await waitFor(() => expect(unlockBiometric).toHaveBeenCalledOnce());
    fireEvent.click(screen.getByText('Mit Touch ID entsperren'));
    expect(unlockBiometric).toHaveBeenCalledTimes(2);
    await waitFor(() => expect(onSuccess).toHaveBeenCalledTimes(2));
  });

  it('shows an error and does not call onSuccess when the passphrase is wrong', async () => {
    const unlock = vi.fn<(passphrase: string) => Promise<void>>().mockRejectedValue(new Error('bad'));
    const { onSuccess } = renderUnlock({ unlock });
    fireEvent.change(screen.getByPlaceholderText('Passwort'), { target: { value: 'wrong' } });
    fireEvent.click(screen.getByText('Entsperren'));
    expect(await screen.findByRole('alert')).toHaveTextContent('Falsches Passwort');
    expect(onSuccess).not.toHaveBeenCalled();
  });

  it('submits the passphrase on Enter and cancels on Escape', async () => {
    const { unlock, onCancel } = renderUnlock();
    const input = screen.getByPlaceholderText('Passwort');
    fireEvent.change(input, { target: { value: 'my-pass' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(unlock).toHaveBeenCalledWith('my-pass');
    fireEvent.keyDown(input, { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it('calls onCancel when the cancel button or the backdrop is clicked', () => {
    const { onCancel } = renderUnlock();
    fireEvent.click(screen.getByText('Abbrechen'));
    expect(onCancel).toHaveBeenCalledOnce();
  });

  it('calls onCancel when clicking the backdrop, but not the dialog panel itself', () => {
    const { onCancel } = renderUnlock();
    fireEvent.click(screen.getByText('Tresor entsperren').closest('div.fixed')!);
    expect(onCancel).toHaveBeenCalledOnce();
    onCancel.mockClear();
    fireEvent.click(screen.getByText('Tresor entsperren'));
    expect(onCancel).not.toHaveBeenCalled();
  });

  it('switches to recovery mode and unlocks via the recovery key', async () => {
    const { unlockRecovery, onSuccess } = renderUnlock();
    fireEvent.click(screen.getByText('Wiederherstellungs-Schlüssel verwenden'));
    const input = screen.getByPlaceholderText('Wiederherstellungs-Schlüssel');
    fireEvent.change(input, { target: { value: 'ABCDE-FGHIJ' } });
    fireEvent.click(screen.getByText('Entsperren'));
    expect(unlockRecovery).toHaveBeenCalledWith('ABCDE-FGHIJ');
    await waitFor(() => expect(onSuccess).toHaveBeenCalledOnce());
  });

  it('shows an error and can switch back to passphrase mode when the recovery key is wrong', async () => {
    const unlockRecovery = vi.fn<(recovery: string) => Promise<void>>().mockRejectedValue(new Error('bad'));
    renderUnlock({ unlockRecovery });
    fireEvent.click(screen.getByText('Wiederherstellungs-Schlüssel verwenden'));
    fireEvent.change(screen.getByPlaceholderText('Wiederherstellungs-Schlüssel'), { target: { value: 'wrong' } });
    fireEvent.click(screen.getByText('Entsperren'));
    expect(await screen.findByRole('alert')).toHaveTextContent('Falsches Passwort');

    fireEvent.click(screen.getByText('Passwort verwenden'));
    expect(screen.getByPlaceholderText('Passwort')).toBeInTheDocument();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('submits the recovery key on Enter and cancels on Escape', async () => {
    const { unlockRecovery, onCancel } = renderUnlock();
    fireEvent.click(screen.getByText('Wiederherstellungs-Schlüssel verwenden'));
    const input = screen.getByPlaceholderText('Wiederherstellungs-Schlüssel');
    fireEvent.change(input, { target: { value: 'ABCDE-FGHIJ' } });
    fireEvent.keyDown(input, { key: 'Enter' });
    expect(unlockRecovery).toHaveBeenCalledWith('ABCDE-FGHIJ');
    fireEvent.keyDown(input, { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledOnce();
  });
});

describe('VaultUnlock — auto-trigger Touch ID on mount', () => {
  it('calls unlockBiometric once automatically when biometric is available', async () => {
    const { unlockBiometric, onSuccess } = renderUnlock({ biometricAvailable: true });
    await waitFor(() => expect(unlockBiometric).toHaveBeenCalledOnce());
    await waitFor(() => expect(onSuccess).toHaveBeenCalledOnce());
  });

  it('does not call unlockBiometric when biometric is unavailable', async () => {
    const { unlockBiometric, onSuccess } = renderUnlock({ biometricAvailable: false });
    await new Promise(r => setTimeout(r, 0));
    expect(unlockBiometric).not.toHaveBeenCalled();
    expect(onSuccess).not.toHaveBeenCalled();
  });

  it('leaves the dialog open with an error when the automatic attempt is rejected, without retrying', async () => {
    const unlock = vi.fn<(passphrase: string) => Promise<void>>().mockResolvedValue(undefined);
    const unlockRecovery = vi.fn<(recovery: string) => Promise<void>>().mockResolvedValue(undefined);
    const unlockBiometric = vi.fn<() => Promise<void>>().mockRejectedValue(new Error('cancelled'));
    const onSuccess = vi.fn();
    const onCancel = vi.fn();
    render(
      <VaultUnlock
        biometricAvailable={true}
        unlock={unlock}
        unlockRecovery={unlockRecovery}
        unlockBiometric={unlockBiometric}
        onSuccess={onSuccess}
        onCancel={onCancel}
      />,
    );

    await waitFor(() => expect(unlockBiometric).toHaveBeenCalledOnce());
    expect(await screen.findByText('Touch ID fehlgeschlagen — bitte Passwort eingeben')).toBeInTheDocument();
    expect(onSuccess).not.toHaveBeenCalled();
    // Dialog stays open (title still rendered) and does not auto-retry.
    expect(screen.getByText('Tresor entsperren')).toBeInTheDocument();
    await new Promise(r => setTimeout(r, 0));
    expect(unlockBiometric).toHaveBeenCalledOnce();
  });
  it('asks for the rotation code after unlocking when one is waiting, and redeems it with the same passphrase', async () => {
    const redeemRotation = vi.fn<(code: string, passphrase: string) => Promise<void>>().mockResolvedValue(undefined);
    const { onSuccess } = renderUnlock({ rotationPending: () => Promise.resolve(true), redeemRotation });

    fireEvent.change(screen.getByPlaceholderText('Passwort'), { target: { value: 'my-pass' } });
    fireEvent.click(screen.getByText('Entsperren'));

    expect(await screen.findByText('Wechsel-Code eingeben')).toBeInTheDocument();
    expect(onSuccess).not.toHaveBeenCalled();
    fireEvent.change(screen.getByPlaceholderText('Wechsel-Code'), { target: { value: 'AAAA-BBBB' } });
    fireEvent.click(screen.getByText('Schlüssel wechseln'));
    await waitFor(() => expect(redeemRotation).toHaveBeenCalledWith('AAAA-BBBB', 'my-pass'));
    await waitFor(() => expect(onSuccess).toHaveBeenCalledOnce());
  });

  it('reports a rejected rotation code and keeps the step open', async () => {
    const redeemRotation = vi.fn<(code: string, passphrase: string) => Promise<void>>()
      .mockRejectedValue(new Error('invalid rotation code'));
    const { onSuccess } = renderUnlock({ rotationPending: () => Promise.resolve(true), redeemRotation });

    fireEvent.change(screen.getByPlaceholderText('Passwort'), { target: { value: 'my-pass' } });
    fireEvent.click(screen.getByText('Entsperren'));
    fireEvent.change(await screen.findByPlaceholderText('Wechsel-Code'), { target: { value: 'nope' } });
    fireEvent.click(screen.getByText('Schlüssel wechseln'));

    expect(await screen.findByRole('alert')).toHaveTextContent('Code ist ungültig oder schon eingelöst');
    expect(onSuccess).not.toHaveBeenCalled();
    expect(screen.getByText('Wechsel-Code eingeben')).toBeInTheDocument();
  });

  it('lets the member postpone the rotation code — the vault is already unlocked', async () => {
    const redeemRotation = vi.fn<(code: string, passphrase: string) => Promise<void>>().mockResolvedValue(undefined);
    const { onSuccess, onCancel } = renderUnlock({ rotationPending: () => Promise.resolve(true), redeemRotation });

    fireEvent.change(screen.getByPlaceholderText('Passwort'), { target: { value: 'my-pass' } });
    fireEvent.click(screen.getByText('Entsperren'));
    fireEvent.click(await screen.findByText('Später'));

    expect(onSuccess).toHaveBeenCalledOnce();
    expect(onCancel).not.toHaveBeenCalled();
    expect(redeemRotation).not.toHaveBeenCalled();
  });

  it('closes straight away when no rotation code is waiting', async () => {
    const rotationPending = vi.fn<() => Promise<boolean>>().mockResolvedValue(false);
    const { onSuccess } = renderUnlock({ rotationPending, redeemRotation: vi.fn() });
    fireEvent.change(screen.getByPlaceholderText('Passwort'), { target: { value: 'my-pass' } });
    fireEvent.click(screen.getByText('Entsperren'));
    await waitFor(() => expect(onSuccess).toHaveBeenCalledOnce());
    expect(rotationPending).toHaveBeenCalledOnce();
    expect(screen.queryByText('Wechsel-Code eingeben')).not.toBeInTheDocument();
  });
  it('hands a waiting rotation code back to the caller after a Touch ID unlock', async () => {
    // Touch ID types no passphrase, so the in-dialog step cannot re-wrap
    // anything — the caller prompts for code AND passphrase instead.
    const rotationPending = vi.fn<() => Promise<boolean>>().mockResolvedValue(true);
    const redeemRotation = vi.fn<(code: string, passphrase: string) => Promise<void>>().mockResolvedValue(undefined);
    const { onSuccess } = renderUnlock({ biometricAvailable: true, rotationPending, redeemRotation });

    await waitFor(() => expect(onSuccess).toHaveBeenCalledWith(true));
    expect(screen.queryByPlaceholderText('Wechsel-Code')).not.toBeInTheDocument();
    expect(redeemRotation).not.toHaveBeenCalled();
  });

  it('reports no rotation code back after a Touch ID unlock when none is waiting', async () => {
    const rotationPending = vi.fn<() => Promise<boolean>>().mockResolvedValue(false);
    const { onSuccess } = renderUnlock({ biometricAvailable: true, rotationPending, redeemRotation: vi.fn() });
    await waitFor(() => expect(onSuccess).toHaveBeenCalledWith(false));
  });

  it('postponing the rotation step reports no pending code, so the caller does not re-ask', async () => {
    const redeemRotation = vi.fn<(code: string, passphrase: string) => Promise<void>>().mockResolvedValue(undefined);
    const { onSuccess } = renderUnlock({ rotationPending: () => Promise.resolve(true), redeemRotation });
    fireEvent.change(screen.getByPlaceholderText('Passwort'), { target: { value: 'my-pass' } });
    fireEvent.click(screen.getByText('Entsperren'));
    fireEvent.click(await screen.findByText('Später'));
    expect(onSuccess).toHaveBeenCalledWith();
  });
});
