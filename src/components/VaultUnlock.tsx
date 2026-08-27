import { useState } from 'react';
import { useTranslation } from 'react-i18next';

interface Props {
  biometricAvailable: boolean;
  unlock: (passphrase: string) => Promise<void>;
  unlockRecovery: (recovery: string) => Promise<void>;
  unlockBiometric: () => Promise<void>;
  onSuccess: () => void;
  onCancel: () => void;
}

/**
 * In-app "unlock the vault" dialog: Touch ID (if available), passphrase, or
 * a recovery key as a fallback.
 */
export default function VaultUnlock({ biometricAvailable, unlock, unlockRecovery, unlockBiometric, onSuccess, onCancel }: Props) {
  const { t } = useTranslation();
  const [mode, setMode] = useState<'passphrase' | 'recovery'>('passphrase');
  const [passphrase, setPassphrase] = useState('');
  const [recovery, setRecovery] = useState('');
  const [error, setError] = useState<string | null>(null);

  const submitBiometric = async () => {
    setError(null);
    try {
      await unlockBiometric();
      onSuccess();
    } catch {
      setError(t('vault.biometricFailed'));
    }
  };

  const submitPassphrase = async () => {
    setError(null);
    try {
      await unlock(passphrase);
      onSuccess();
    } catch {
      setError(t('vault.wrongPassphrase'));
    }
  };

  const submitRecovery = async () => {
    setError(null);
    try {
      await unlockRecovery(recovery);
      onSuccess();
    } catch {
      setError(t('vault.wrongPassphrase'));
    }
  };

  const switchMode = (next: 'passphrase' | 'recovery') => {
    setMode(next);
    setError(null);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }} onClick={onCancel}>
      <div className="w-96 rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
        <h2 className="text-gray-100 text-base font-semibold mb-3">{t('vault.unlockTitle')}</h2>

        {biometricAvailable && mode === 'passphrase' && (
          <button
            onClick={() => void submitBiometric()}
            className="w-full mb-3 px-3 py-2 rounded text-sm font-medium"
            style={{ background: 'var(--line)', color: '#1c1917' }}
          >
            {t('vault.unlockTouchId')}
          </button>
        )}

        {mode === 'passphrase' ? (
          <input
            type="password"
            autoFocus
            value={passphrase}
            placeholder={t('vault.passphrase')}
            onChange={e => setPassphrase(e.target.value)}
            onKeyDown={e => { if (e.key === 'Enter') void submitPassphrase(); else if (e.key === 'Escape') onCancel(); }}
            className="w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-gray-100 outline-none focus:border-[var(--accent)] mb-2"
          />
        ) : (
          <input
            autoFocus
            value={recovery}
            placeholder={t('vault.recoveryKey')}
            onChange={e => setRecovery(e.target.value)}
            onKeyDown={e => { if (e.key === 'Enter') void submitRecovery(); else if (e.key === 'Escape') onCancel(); }}
            className="w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-gray-100 outline-none focus:border-[var(--accent)] mb-2 font-mono"
          />
        )}

        {error && (
          <div className="text-sm text-red-400 mb-2" role="alert">
            {error}
          </div>
        )}

        <button
          onClick={() => switchMode(mode === 'passphrase' ? 'recovery' : 'passphrase')}
          className="text-xs text-gray-400 hover:text-gray-200 underline mb-3"
        >
          {mode === 'passphrase' ? t('vault.useRecovery') : t('vault.usePassphrase')}
        </button>

        <div className="flex justify-end gap-2">
          <button onClick={onCancel} className="px-3 py-1.5 rounded text-sm text-gray-300 hover:bg-gray-800">
            {t('vault.cancel')}
          </button>
          <button
            onClick={() => void (mode === 'passphrase' ? submitPassphrase() : submitRecovery())}
            className="px-3 py-1.5 rounded text-sm font-medium"
            style={{ background: 'var(--line)', color: '#1c1917' }}
          >
            {t('vault.unlock')}
          </button>
        </div>
      </div>
    </div>
  );
}
