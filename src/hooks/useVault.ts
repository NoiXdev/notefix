import { useCallback, useEffect, useState } from 'react';
import { api } from '../api';
import type { VaultStatus } from '../types';

export function useVault() {
  const [status, setStatus] = useState<VaultStatus>({ exists: false, unlocked: false, biometric: false, conflict: false, recoveryHolder: true });

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

  return { status, refresh, setup, unlock, unlockRecovery, unlockBiometric, lock, changePassphrase };
}
