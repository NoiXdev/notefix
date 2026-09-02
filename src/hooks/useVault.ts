import { useCallback, useEffect, useState } from 'react';
import { api } from '../api';
import type { VaultStatus } from '../types';

export function useVault() {
  const [status, setStatus] = useState<VaultStatus>({ exists: false, unlocked: false, biometric: false, conflict: false, recoveryHolder: true, rotationCode: false, recoveryMissing: false });

  const refresh = useCallback(async () => {
    setStatus(await api.vault.status());
  }, []);

  useEffect(() => {
    void refresh();
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
