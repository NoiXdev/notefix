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

  it('shows a Touch ID button that calls unlockBiometric when available', async () => {
    const { unlockBiometric, onSuccess } = renderUnlock({ biometricAvailable: true });
    fireEvent.click(screen.getByText('Mit Touch ID entsperren'));
    expect(unlockBiometric).toHaveBeenCalledOnce();
    await waitFor(() => expect(onSuccess).toHaveBeenCalledOnce());
  });
});
