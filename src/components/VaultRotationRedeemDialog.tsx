import { useState } from 'react';
import { useTranslation } from 'react-i18next';

interface Props {
  redeem: (code: string, passphrase: string) => Promise<void>;
  onSuccess: () => void;
  onCancel: () => void;
}

/**
 * Redeems a one-time rotation code: the code the workspace owner handed over,
 * plus the passphrase this member already unlocks with — the code only opens
 * the new key, and it is immediately re-wrapped under that passphrase.
 *
 * The standalone counterpart of `VaultUnlock`'s rotation step, for every way
 * in that does NOT type a passphrase: a Touch ID unlock, and the Security page
 * banner for a member who postponed the step.
 */
export default function VaultRotationRedeemDialog({ redeem, onSuccess, onCancel }: Props) {
  const { t } = useTranslation();
  const [code, setCode] = useState('');
  const [passphrase, setPassphrase] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    setError(null);
    setBusy(true);
    try {
      await redeem(code, passphrase);
      onSuccess();
    } catch (e) {
      // A locked vault refuses before the code is even looked at, so it must
      // never be reported as "code burnt" — the one-time code is still good.
      const msg = e instanceof Error ? e.message : String(e ?? '');
      if (msg.includes('vault locked')) setError(t('vault.rotation.lockedHint'));
      else if (msg.includes('wrong passphrase')) setError(t('vault.wrongPassphrase'));
      else setError(t('vault.rotation.invalidCode'));
    } finally {
      setBusy(false);
    }
  };

  // Exactly the condition the submit button is disabled on, so Enter cannot
  // fire a request the button refuses — an empty code would come back as
  // "invalid rotation code" and read as if the real code had been spent.
  const canSubmit = !busy && !!code.trim() && !!passphrase;

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') { if (canSubmit) void submit(); }
    else if (e.key === 'Escape') onCancel();
  };
  const field = 'w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-gray-100 outline-none focus:border-[var(--accent)] mb-2';

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }} onClick={onCancel}>
      <div className="w-96 rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
        <h2 className="text-gray-100 text-base font-semibold mb-2">{t('vault.rotation.enterCode')}</h2>
        <p className="text-gray-400 text-sm mb-3">{t('vault.rotation.enterCodeHint')}</p>
        <input
          autoFocus
          value={code}
          placeholder={t('vault.rotation.code')}
          onChange={e => setCode(e.target.value)}
          onKeyDown={onKeyDown}
          className={`${field} font-mono`}
        />
        <input
          type="password"
          value={passphrase}
          placeholder={t('vault.passphrase')}
          onChange={e => setPassphrase(e.target.value)}
          onKeyDown={onKeyDown}
          className={field}
        />
        {error && (
          <div className="text-sm text-red-400 mb-2" role="alert">
            {error}
          </div>
        )}
        <div className="flex justify-end gap-2 mt-2">
          <button onClick={onCancel} className="px-3 py-1.5 rounded text-sm text-gray-300 hover:bg-gray-800">
            {t('vault.rotation.later')}
          </button>
          <button
            onClick={() => void submit()}
            disabled={!canSubmit}
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
