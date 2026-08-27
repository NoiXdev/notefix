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
});
