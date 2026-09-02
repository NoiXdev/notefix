import { useCallback, useEffect, useState } from 'react';
import { api } from '../api';
import type { VaultStatus } from '../types';

export function useVault() {
  const [status, setStatus] = useState<VaultStatus>({ exists: false, unlocked: false, biometric: false, conflict: false, recoveryHolder: true, rotationCode: false, recoveryMissing: false, sealOutdated: false });

  const refresh = useCallback(async () => {
    try {
      setStatus(await api.vault.status());
    } catch {
      // No active context yet, a store being swapped, a backend hiccup —
      // none of them is worth an unhandled rejection. The status simply
      // stays as it was; the next refresh (or `context-changed`) corrects it.
    }
  }, []);

  useEffect(() => {
    void refresh();
    // Switching contexts locks the vault backend-side (the DEK belongs to the
    // previous context's DB), and every vault action the backend performs on
    // its own — an accepted invitation, a rotation — broadcasts the same
    // event. Subscribing HERE rather than in each consumer is what keeps the
    // Security and Contexts pages and their banners from going stale.
    return api.onContextChanged(() => { void refresh(); });
  }, [refresh]);

  const setup = useCallback(
    async (passphrase: string) => {
      const recoveryCodes = await api.vault.setup(passphrase);
      await refresh();
      return recoveryCodes;
    },
    [refresh],
  );

  const unlock = useCallback(
    async (passphrase: string) => {
      await api.vault.unlock(passphrase);
      await refresh();
    },
    [refresh],
  );

  const unlockRecovery = useCallback(
    async (recovery: string) => {
      await api.vault.unlockRecovery(recovery);
      await refresh();
    },
    [refresh],
  );

  const unlockBiometric = useCallback(async () => {
    await api.vault.unlockBiometric();
    await refresh();
  }, [refresh]);

  const lock = useCallback(async () => {
    await api.vault.lock();
    await refresh();
  }, [refresh]);

  const changePassphrase = useCallback(
    async (current: string, next: string) => {
      await api.vault.changePassphrase(current, next);
      await refresh();
    },
    [refresh],
  );

  /** Redeem the one-time rotation code the workspace owner handed out. */
  const redeemRotation = useCallback(
    async (code: string, passphrase: string) => {
      await api.vault.rotationRedeem(code, passphrase);
      await refresh();
    },
    [refresh],
  );

  /** Creator-only: add the recovery wrap for a generation someone else rotated. */
  const recoveryFollowup = useCallback(
    async (recoveryKey: string) => {
      await api.vault.recoveryFollowup(recoveryKey);
      await refresh();
    },
    [refresh],
  );

  return { status, refresh, setup, unlock, unlockRecovery, unlockBiometric, lock, changePassphrase, redeemRotation, recoveryFollowup };
}
