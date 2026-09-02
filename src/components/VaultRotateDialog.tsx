import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { RotationCode } from '../types';

interface Props {
  /**
   * Whether this user holds the vault's recovery key. Only they can carry it
   * over to the new key generation, so only they are asked for it — for
   * everyone else the vault's creator adds it afterwards.
   */
  recoveryHolder: boolean;
  rotate: (passphrase: string, recoveryKey?: string) => Promise<RotationCode[]>;
  onSuccess: (codes: RotationCode[]) => void;
  onCancel: () => void;
}

/**
 * Collects what a key rotation needs before anything is minted: the
 * passphrase this user's own new wrap is built under, and — for the vault's
 * creator — the recovery key that keeps the recovery path working across the
 * new generation. Both are verified by the backend against the cached wraps,
 * so a typo fails here rather than stranding the workspace.
 */
export default function VaultRotateDialog({ recoveryHolder, rotate, onSuccess, onCancel }: Props) {
  const { t } = useTranslation();
  const [passphrase, setPassphrase] = useState('');
  const [recoveryKey, setRecoveryKey] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setError(null);
    setBusy(true);
    try {
      onSuccess(await rotate(passphrase, recoveryHolder ? recoveryKey : undefined));
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e ?? '');
      if (msg.includes('wrong passphrase')) setError(t('vault.wrongPassphrase'));
      else if (msg.includes('recovery key')) setError(t('vault.rotation.failed', { error: msg }));
      else if (msg.includes('no rotation pending')) setError(t('vault.rotation.noPending'));
      else setError(t('vault.rotation.failed', { error: msg }));
    } finally {
      setBusy(false);
    }
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') void submit();
    else if (e.key === 'Escape') onCancel();
  };
  const field = 'w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-gray-100 outline-none focus:border-[var(--accent)] mb-2';

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }} onClick={onCancel}>
      <div className="w-96 rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
        <h2 className="text-gray-100 text-base font-semibold mb-2">{t('vault.rotation.title')}</h2>
        <p className="text-gray-400 text-sm mb-3">{t('vault.rotation.hint')}</p>
        <input
          type="password"
          autoFocus
          value={passphrase}
          placeholder={t('vault.passphrase')}
          onChange={e => setPassphrase(e.target.value)}
          onKeyDown={onKeyDown}
          className={field}
        />
        {recoveryHolder && (
          <>
            <p className="text-gray-400 text-xs mb-2">{t('vault.rotation.recoveryHint')}</p>
            <input
              value={recoveryKey}
              placeholder={t('vault.recoveryKey')}
              onChange={e => setRecoveryKey(e.target.value)}
              onKeyDown={onKeyDown}
              className={`${field} font-mono`}
            />
          </>
        )}
        {error && (
          <div className="text-sm text-red-400 mb-2" role="alert">
            {error}
          </div>
        )}
        <div className="flex justify-end gap-2 mt-2">
          <button onClick={onCancel} className="px-3 py-1.5 rounded text-sm text-gray-300 hover:bg-gray-800">
            {t('vault.cancel')}
          </button>
          <button
            onClick={() => void submit()}
            disabled={busy || !passphrase || (recoveryHolder && !recoveryKey.trim())}
            className="px-3 py-1.5 rounded text-sm font-medium disabled:opacity-40"
            style={{ background: 'var(--line)', color: '#1c1917' }}
          >
            {t('vault.rotation.submit')}
          </button>
        </div>
      </div>
    </div>
  );
}
