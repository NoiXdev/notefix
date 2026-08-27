import { useState } from 'react';
import { useTranslation } from 'react-i18next';

interface Props {
  setup: (passphrase: string) => Promise<string[]>;
  onSuccess: () => void;
  onCancel: () => void;
}

/**
 * In-app "create the vault" dialog: passphrase + confirm, then a recovery-key
 * screen the user must acknowledge before onSuccess fires.
 */
export default function VaultSetup({ setup, onSuccess, onCancel }: Props) {
  const { t } = useTranslation();
  const [passphrase, setPassphrase] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [recoveryGroups, setRecoveryGroups] = useState<string[] | null>(null);
  const [copied, setCopied] = useState(false);

  const submit = async () => {
    if (!passphrase || !confirm || passphrase !== confirm) {
      setError(t('vault.mismatch'));
      return;
    }
    setError(null);
    try {
      setRecoveryGroups(await setup(passphrase));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const copy = () => {
    if (!recoveryGroups) return;
    void navigator.clipboard?.writeText(recoveryGroups.join('-'));
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
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
          <div className="flex justify-end gap-2">
            <button onClick={copy} className="px-3 py-1.5 rounded text-sm text-gray-300 hover:bg-gray-800">
              {copied ? t('vault.copied') : t('vault.copy')}
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
