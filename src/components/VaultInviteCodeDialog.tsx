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
  const [copied, setCopied] = useState(false);

  const copy = () => {
    void navigator.clipboard?.writeText(code);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
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
        <div className="flex justify-end gap-2">
          <button onClick={copy} className="px-3 py-1.5 rounded text-sm text-gray-300 hover:bg-gray-800">
            {copied ? t('vault.invite.copied') : t('vault.invite.copy')}
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
