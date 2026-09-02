import { useState } from 'react';
import { useTranslation } from 'react-i18next';

interface Props {
  setup: (passphrase: string) => Promise<string[]>;
  onSuccess: () => void;
  onCancel: () => void;
  /**
   * Called when the backend says the workspace ALREADY has a vault — another
   * device seeded it, or this one only ever pulled the wrapped key. Setting
   * up is then the wrong door entirely: the caller is expected to send the
   * user to the unlock dialog instead. Optional, so a caller that has no
   * unlock dialog to offer (the Security page) simply shows the message.
   */
  onAlreadyExists?: () => void;
}

/**
 * In-app "create the vault" dialog: passphrase + confirm, then a recovery-key
 * screen the user must acknowledge before onSuccess fires.
 */
export default function VaultSetup({ setup, onSuccess, onCancel, onAlreadyExists }: Props) {
  const { t } = useTranslation();
  const [passphrase, setPassphrase] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [recoveryGroups, setRecoveryGroups] = useState<string[] | null>(null);
  const [copyState, setCopyState] = useState<'idle' | 'copied' | 'failed'>('idle');

  const submit = async () => {
    if (!passphrase || !confirm || passphrase !== confirm) {
      setError(t('vault.mismatch'));
      return;
    }
    setError(null);
    try {
      setRecoveryGroups(await setup(passphrase));
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err ?? '');
      if (msg.includes('already set up on the server')) {
        setError(t('vault.alreadyOnServer'));
        onAlreadyExists?.();
        return;
      }
      // Everything else — an unreachable server above all — keeps the
      // backend's own words, but inside a sentence the user can read.
      setError(t('vault.setupFailed', { error: msg }));
    }
  };

  const copy = async () => {
    if (!recoveryGroups) return;
    try {
      await navigator.clipboard.writeText(recoveryGroups.join('-'));
      setCopyState('copied');
    } catch {
      // The recovery key is shown exactly once. Claiming "Kopiert" over a
      // refused clipboard write would have the user dismiss the only copy
      // that will ever exist.
      setCopyState('failed');
      return;
    }
    setTimeout(() => setCopyState('idle'), 1500);
  };

  if (recoveryGroups) {
    return (
      <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }}>
        <div className="w-96 rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
          <h2 className="text-gray-100 text-base font-semibold mb-2">{t('vault.recoveryTitle')}</h2>
          <p className="text-gray-400 text-sm mb-3">{t('vault.recoveryHint')}</p>
          <div className="font-mono text-sm text-gray-100 bg-gray-800 border border-gray-700 rounded px-3 py-2 mb-4 break-all">
            {recoveryGroups.join('-')}
          </div>
          {copyState === 'failed' && (
            <div className="text-sm text-red-400 mb-2" role="alert">{t('common.copyFailed')}</div>
          )}
          <div className="flex justify-end gap-2">
            <button onClick={() => void copy()} className="px-3 py-1.5 rounded text-sm text-gray-300 hover:bg-gray-800">
              {copyState === 'copied' ? t('vault.copied') : t('vault.copy')}
            </button>
            <button
              onClick={onSuccess}
              className="px-3 py-1.5 rounded text-sm font-medium"
              style={{ background: 'var(--line)', color: '#1c1917' }}
            >
              {t('vault.savedIt')}
            </button>
          </div>
        </div>
      </div>
    );
  }

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') void submit();
    else if (e.key === 'Escape') onCancel();
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }} onClick={onCancel}>
      <div className="w-96 rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
        <h2 className="text-gray-100 text-base font-semibold mb-3">{t('vault.setupTitle')}</h2>
        <input
          type="password"
          autoFocus
          value={passphrase}
          placeholder={t('vault.passphrase')}
          onChange={e => setPassphrase(e.target.value)}
          onKeyDown={onKeyDown}
          className="w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-gray-100 outline-none focus:border-[var(--accent)] mb-2"
        />
        <input
          type="password"
          value={confirm}
          placeholder={t('vault.confirmPassphrase')}
          onChange={e => setConfirm(e.target.value)}
          onKeyDown={onKeyDown}
          className="w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-gray-100 outline-none focus:border-[var(--accent)] mb-2"
        />
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
            disabled={!passphrase || !confirm}
            className="px-3 py-1.5 rounded text-sm font-medium disabled:opacity-40"
            style={{ background: 'var(--line)', color: '#1c1917' }}
          >
            {t('vault.create')}
          </button>
        </div>
      </div>
    </div>
  );
}
