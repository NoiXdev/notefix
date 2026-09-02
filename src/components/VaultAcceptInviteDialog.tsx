import { useState } from 'react';
import { useTranslation } from 'react-i18next';

interface Props {
  /** Pasted link or id -> the numeric invitation id (a server round-trip). */
  resolve: (reference: string) => Promise<number>;
  accept: (invitationId: number, code: string, passphrase: string) => Promise<void>;
  onSuccess: () => void;
  onCancel: () => void;
}

/**
 * In-app "join the workspace vault" dialog: the invitation the owner shared,
 * the one-time code they read out, and the passphrase this member will unlock
 * with from then on.
 *
 * The invitation field takes the share link as pasted — nobody ever sees the
 * numeric id — so it is resolved first, then redeemed. A failed lookup and a
 * rejected code are reported apart: one means "wrong link", the other "wrong
 * code", and telling them apart is the difference between a user retrying the
 * right field or the wrong one.
 */
export default function VaultAcceptInviteDialog({ resolve, accept, onSuccess, onCancel }: Props) {
  const { t } = useTranslation();
  const [reference, setReference] = useState('');
  const [code, setCode] = useState('');
  const [passphrase, setPassphrase] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // The accept installed a key and unlocked the vault — a silently closing
  // dialog would leave the member wondering whether anything happened.
  const [accepted, setAccepted] = useState(false);

  const submit = async () => {
    if (!passphrase || passphrase !== confirm) {
      setError(t('vault.mismatch'));
      return;
    }
    setError(null);
    setBusy(true);
    let invitationId: number;
    try {
      invitationId = await resolve(reference);
    } catch {
      setBusy(false);
      setError(t('vault.invite.resolveFailed'));
      return;
    }
    try {
      await accept(invitationId, code, passphrase);
      setAccepted(true);
    } catch {
      setError(t('vault.invite.invalid'));
    } finally {
      setBusy(false);
    }
  };

  if (accepted) {
    return (
      <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }}>
        <div className="w-96 rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
          <h2 className="text-gray-100 text-base font-semibold mb-4">{t('vault.invite.accepted')}</h2>
          <div className="flex justify-end">
            <button
              onClick={onSuccess}
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

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') void submit();
    else if (e.key === 'Escape') onCancel();
  };
  const field = 'w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-gray-100 outline-none focus:border-[var(--accent)] mb-2';

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }} onClick={onCancel}>
      {/* Four fields plus hint and error do not fit a phone in landscape —
          scroll inside the panel rather than off the bottom of the screen. */}
      <div className="w-96 max-h-[90vh] overflow-y-auto rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
        <h2 className="text-gray-100 text-base font-semibold mb-2">{t('vault.invite.enter')}</h2>
        <p className="text-gray-400 text-sm mb-3">{t('vault.invite.enterHint')}</p>
        <input
          autoFocus
          value={reference}
          placeholder={t('vault.invite.reference')}
          onChange={e => setReference(e.target.value)}
          onKeyDown={onKeyDown}
          className={field}
        />
        <input
          value={code}
          placeholder={t('vault.invite.code')}
          onChange={e => setCode(e.target.value)}
          onKeyDown={onKeyDown}
          className={`${field} font-mono`}
        />
        <input
          type="password"
          value={passphrase}
          placeholder={t('vault.invite.newPassphrase')}
          onChange={e => setPassphrase(e.target.value)}
          onKeyDown={onKeyDown}
          className={field}
        />
        <input
          type="password"
          value={confirm}
          placeholder={t('vault.invite.confirmPassphrase')}
          onChange={e => setConfirm(e.target.value)}
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
            {t('vault.cancel')}
          </button>
          <button
            onClick={() => void submit()}
            disabled={busy || !reference.trim() || !code.trim() || !passphrase || !confirm}
            className="px-3 py-1.5 rounded text-sm font-medium disabled:opacity-40"
            style={{ background: 'var(--line)', color: '#1c1917' }}
          >
            {t('vault.invite.submit')}
          </button>
        </div>
      </div>
    </div>
  );
}
