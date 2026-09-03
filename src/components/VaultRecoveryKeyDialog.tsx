import { useState } from 'react';
import { useTranslation } from 'react-i18next';

interface Props { groups: string[]; onClose: () => void; }

/**
 * Shows a freshly created recovery key exactly once. The close button stays
 * disabled until the user confirms they stored the key — after this dialog
 * the key exists nowhere in the app.
 */
export default function VaultRecoveryKeyDialog({ groups, onClose }: Props) {
  const { t } = useTranslation();
  const [copyState, setCopyState] = useState<'idle' | 'copied' | 'failed'>('idle');
  const [saved, setSaved] = useState(false);
  const key = groups.join('-');
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(key);
      setCopyState('copied');
    } catch {
      setCopyState('failed');
    }
  };
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }}>
      <div className="w-96 rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
        <h2 className="text-gray-100 text-base font-semibold mb-2">{t('vault.recovery.createdTitle')}</h2>
        <p className="text-gray-400 text-sm mb-3">{t('vault.recoveryHint')}</p>
        <div className="font-mono text-sm text-gray-100 bg-gray-800 border border-gray-700 rounded px-3 py-2 mb-3 break-all">{key}</div>
        {copyState === 'failed' && <div className="text-sm text-red-400 mb-2" role="alert">{t('common.copyFailed')}</div>}
        <label className="flex items-center gap-2 text-sm text-gray-200 mb-4">
          <input type="checkbox" checked={saved} onChange={e => setSaved(e.target.checked)} aria-label={t('vault.recovery.savedCheckbox')} />
          <span>{t('vault.recovery.savedCheckbox')}</span>
        </label>
        <div className="flex justify-end gap-2">
          <button onClick={() => void copy()} className="px-3 py-1.5 rounded text-sm text-gray-300 hover:bg-gray-800">
            {copyState === 'copied' ? t('vault.copied') : t('vault.copy')}
          </button>
          <button onClick={onClose} disabled={!saved} className="px-3 py-1.5 rounded text-sm font-medium disabled:opacity-40" style={{ background: 'var(--line)', color: '#1c1917' }}>
            {t('vault.recovery.savedIt')}
          </button>
        </div>
      </div>
    </div>
  );
}
