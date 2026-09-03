import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { RotationCode } from '../types';

interface Props {
  codes: RotationCode[];
  onClose: () => void;
}

/**
 * Shows the one-time rotation codes a key change just minted — one per
 * remaining member, each the only thing that can open that member's new
 * wrapped key.
 *
 * Like the invite code, they are shown once and stored nowhere: closing this
 * dialog is the last time they exist in the app. A member who loses theirs
 * needs another rotation.
 */
export default function VaultRotationCodesDialog({ codes, onClose }: Props) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState<number | null>(null);
  // Which member's copy failed, if any — see `VaultInviteCodeDialog`: a code
  // is shown once, so a silent clipboard failure loses it for good.
  const [failed, setFailed] = useState<number | null>(null);

  const label = (c: RotationCode) => (c.name.trim() ? c.name : t('vault.rotation.codeFor', { id: c.userId }));

  const copy = async (c: RotationCode) => {
    try {
      await navigator.clipboard.writeText(c.code);
    } catch {
      setCopied(null);
      setFailed(c.userId);
      return;
    }
    setFailed(null);
    setCopied(c.userId);
    setTimeout(() => setCopied(null), 1500);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }}>
      <div className="w-96 rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
        <h2 className="text-gray-100 text-base font-semibold mb-2">{t('vault.rotation.codesTitle')}</h2>
        <p className="text-gray-400 text-sm mb-3">{t('vault.rotation.codesHint')}</p>
        <div className="max-h-64 overflow-y-auto mb-4">
          {codes.map(c => (
            <div key={c.userId} className="mb-3">
              <div className="flex items-center justify-between gap-2 mb-1">
                <span className="text-xs text-gray-400">{label(c)}</span>
                <button
                  onClick={() => void copy(c)}
                  aria-label={`${t('vault.invite.copy')} — ${label(c)}`}
                  className="px-2 py-0.5 rounded text-xs text-gray-300 hover:bg-gray-800"
                >
                  {copied === c.userId ? t('vault.invite.copied') : t('vault.invite.copy')}
                </button>
              </div>
              <div className="font-mono text-sm text-gray-100 bg-gray-800 border border-gray-700 rounded px-3 py-2 break-all">
                {c.code}
              </div>
              {failed === c.userId && (
                <div className="text-xs text-red-400 mt-1" role="alert">{t('common.copyFailed')}</div>
              )}
            </div>
          ))}
          {codes.length === 0 && <p className="text-gray-400 text-sm">{t('vault.rotation.done')}</p>}
        </div>
        <div className="flex justify-end">
          <button
            onClick={onClose}
            className="px-3 py-1.5 rounded text-sm font-medium"
            style={{ background: 'var(--line)', color: '#1c1917' }}
          >
            {t('vault.invite.done')}
          </button>
        </div>
      </div>
    </div>
  );
}
