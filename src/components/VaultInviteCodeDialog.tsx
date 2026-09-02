import { useState } from 'react';
import { useTranslation } from 'react-i18next';

interface Props {
  code: string;
  onClose: () => void;
}

/**
 * Shows the one-time invite code the owner just minted, once. The code is the
 * only thing that can open the wrapped vault key sitting on the invitation, so
 * it is deliberately never persisted anywhere: closing this dialog is the last
 * time it exists in the app.
 *
 * Mirrors the recovery-key screen of `VaultSetup` — monospace box, copy
 * button, acknowledge to dismiss.
 */
export default function VaultInviteCodeDialog({ code, onClose }: Props) {
  const { t } = useTranslation();
  // 'idle' | 'copied' | 'failed'. A clipboard write can be refused (no
  // permission, no clipboard API in this WebView at all) and reporting
  // "Kopiert" over a failure would have the owner close the dialog believing
  // they still have the code — which is shown exactly once.
  const [copyState, setCopyState] = useState<'idle' | 'copied' | 'failed'>('idle');

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopyState('copied');
    } catch {
      setCopyState('failed');
      return;
    }
    setTimeout(() => setCopyState('idle'), 1500);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }}>
      <div className="w-96 rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
        <h2 className="text-gray-100 text-base font-semibold mb-2">{t('vault.invite.codeTitle')}</h2>
        <p className="text-gray-400 text-sm mb-3">{t('vault.invite.codeHint')}</p>
        <div className="font-mono text-sm text-gray-100 bg-gray-800 border border-gray-700 rounded px-3 py-2 mb-3 break-all">
          {code}
        </div>
        <p className="text-gray-400 text-xs mb-4">{t('vault.invite.shareHint')}</p>
        {copyState === 'failed' && (
          <div className="text-sm text-red-400 mb-2" role="alert">{t('common.copyFailed')}</div>
        )}
        <div className="flex justify-end gap-2">
          <button onClick={() => void copy()} className="px-3 py-1.5 rounded text-sm text-gray-300 hover:bg-gray-800">
            {copyState === 'copied' ? t('vault.invite.copied') : t('vault.invite.copy')}
          </button>
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
