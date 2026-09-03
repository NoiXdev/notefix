import { useState } from 'react';
import { useTranslation } from 'react-i18next';

interface CodeEntry {
  id: string;
  label: string;
  code: string;
}

interface Props {
  title: string;
  hint: string;
  emptyText: string;
  entries: CodeEntry[];
  onClose: () => void;
}

/**
 * Shows a batch of one-time vault codes minted just now — one per entry,
 * each the only thing that can open that entry's wrapped key. Shared by the
 * key-rotation flow (one code per remaining member) and the invitation
 * re-code flow (one code per invitation whose wrap a rotation retired).
 *
 * Like the single invite code, they are shown once and stored nowhere:
 * closing this dialog is the last time they exist in the app. Losing one
 * needs another rotation or re-code.
 */
export default function VaultCodesDialog({ title, hint, emptyText, entries, onClose }: Props) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState<string | null>(null);
  // Which entry's copy failed, if any — see `VaultInviteCodeDialog`: a code
  // is shown once, so a silent clipboard failure loses it for good.
  const [failed, setFailed] = useState<string | null>(null);

  const copy = async (e: CodeEntry) => {
    try {
      await navigator.clipboard.writeText(e.code);
    } catch {
      setCopied(null);
      setFailed(e.id);
      return;
    }
    setFailed(null);
    setCopied(e.id);
    setTimeout(() => setCopied(null), 1500);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }}>
      <div className="w-96 rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
        <h2 className="text-gray-100 text-base font-semibold mb-2">{title}</h2>
        <p className="text-gray-400 text-sm mb-3">{hint}</p>
        <div className="max-h-64 overflow-y-auto mb-4">
          {entries.map(e => (
            <div key={e.id} className="mb-3">
              <div className="flex items-center justify-between gap-2 mb-1">
                <span className="text-xs text-gray-400">{e.label}</span>
                <button
                  onClick={() => void copy(e)}
                  aria-label={`${t('vault.invite.copy')} — ${e.label}`}
                  className="px-2 py-0.5 rounded text-xs text-gray-300 hover:bg-gray-800"
                >
                  {copied === e.id ? t('vault.invite.copied') : t('vault.invite.copy')}
                </button>
              </div>
              <div className="font-mono text-sm text-gray-100 bg-gray-800 border border-gray-700 rounded px-3 py-2 break-all">
                {e.code}
              </div>
              {failed === e.id && (
                <div className="text-xs text-red-400 mt-1" role="alert">{t('common.copyFailed')}</div>
              )}
            </div>
          ))}
          {entries.length === 0 && <p className="text-gray-400 text-sm">{emptyText}</p>}
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
