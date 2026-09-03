import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { ConflictOutcome } from '../types';

type LocalSecret = { kind: 'passphrase' | 'recovery'; value: string };
type Mode = 'merge' | 'unprotect';

interface Props {
  resolve: (workspacePassphrase: string, localSecret: LocalSecret, mode: Mode) => Promise<ConflictOutcome>;
  onClose: () => void;
}

/**
 * Resolves a vault conflict: the workspace vault's passphrase opens the shared
 * key, the device's own passphrase (or recovery key) opens the notes sealed
 * before joining, and the user chooses whether those notes move into the
 * workspace vault or become plain notes. Either way every member can read
 * them afterwards — the dialog says so before anything happens.
 */
export default function VaultConflictDialog({ resolve, onClose }: Props) {
  const { t } = useTranslation();
  const [workspacePassphrase, setWorkspacePassphrase] = useState('');
  const [localValue, setLocalValue] = useState('');
  const [localKind, setLocalKind] = useState<'passphrase' | 'recovery'>('passphrase');
  const [mode, setMode] = useState<Mode>('merge');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [outcome, setOutcome] = useState<ConflictOutcome | null>(null);

  const submit = async () => {
    setError(null);
    setBusy(true);
    try {
      setOutcome(await resolve(workspacePassphrase, { kind: localKind, value: localValue }, mode));
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e ?? '');
      if (msg.includes('local record does not open')) setError(t('vault.conflict.localWrong'));
      else if (msg.includes('wrong passphrase')) setError(t('vault.wrongPassphrase'));
      else if (msg.includes('context changed during the request')) setError(t('common.contextChanged'));
      else setError(t('vault.conflict.failed', { error: msg }));
    } finally {
      setBusy(false);
    }
  };

  // Both secrets are trimmed the same way: a passphrase of nothing but
  // spaces cannot open a record either, so enabling the button for one and
  // not the other only buys a round trip through Argon2.
  const canSubmit = !busy && !!workspacePassphrase.trim() && !!localValue.trim();
  // While the resolution runs, every way out is inert: it re-seals notes and
  // rewrites this device's record, and closing the dialog would hide a result
  // the user needs to see (and an error they need to act on).
  const cancel = () => { if (!busy) onClose(); };
  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') { if (canSubmit) void submit(); }
    else if (e.key === 'Escape') cancel();
  };
  const field = 'w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-gray-100 outline-none focus:border-[var(--accent)] mb-2';

  if (outcome) {
    return (
      <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }}>
        <div className="w-96 rounded-lg bg-gray-900 border border-gray-700 p-5" role="status" onClick={e => e.stopPropagation()}>
          <h2 className="text-gray-100 text-base font-semibold mb-2">{t('vault.conflict.title')}</h2>
          <p className="text-gray-200 text-sm mb-2">
            {t(mode === 'merge' ? 'vault.conflict.done' : 'vault.conflict.doneUnprotected', { count: outcome.changed })}
          </p>
          {outcome.skipped > 0 && (
            <p className="text-gray-400 text-sm mb-2">{t('vault.conflict.skipped', { count: outcome.skipped })}</p>
          )}
          <div className="flex justify-end mt-2">
            <button onClick={onClose} className="px-3 py-1.5 rounded text-sm font-medium" style={{ background: 'var(--line)', color: '#1c1917' }}>
              {t('vault.invite.done')}
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }} onClick={cancel}>
      <div className="w-[26rem] max-h-[90vh] overflow-y-auto rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
        <h2 className="text-gray-100 text-base font-semibold mb-2">{t('vault.conflict.title')}</h2>
        <p className="text-gray-400 text-sm mb-3">{t('vault.conflict.dialogHint')}</p>
        <input
          autoFocus
          type="password"
          value={workspacePassphrase}
          placeholder={t('vault.conflict.workspacePassphrase')}
          onChange={e => setWorkspacePassphrase(e.target.value)}
          onKeyDown={onKeyDown}
          className={field}
        />
        <input
          type={localKind === 'passphrase' ? 'password' : 'text'}
          value={localValue}
          placeholder={t(localKind === 'passphrase' ? 'vault.conflict.localPassphrase' : 'vault.conflict.localRecovery')}
          onChange={e => setLocalValue(e.target.value)}
          onKeyDown={onKeyDown}
          className={`${field} ${localKind === 'recovery' ? 'font-mono' : ''}`}
        />
        <button
          type="button"
          onClick={() => { setLocalKind(k => (k === 'passphrase' ? 'recovery' : 'passphrase')); setLocalValue(''); }}
          className="text-xs text-gray-400 hover:text-gray-200 mb-3"
        >
          {t(localKind === 'passphrase' ? 'vault.conflict.useRecovery' : 'vault.conflict.usePassphrase')}
        </button>
        <fieldset className="mb-3">
          <label className="flex items-start gap-2 text-sm text-gray-200 mb-1">
            <input type="radio" name="conflict-mode" checked={mode === 'merge'} onChange={() => setMode('merge')} aria-label={t('vault.conflict.modeMerge')} />
            <span>{t('vault.conflict.modeMerge')}</span>
          </label>
          <label className="flex items-start gap-2 text-sm text-gray-200">
            <input type="radio" name="conflict-mode" checked={mode === 'unprotect'} onChange={() => setMode('unprotect')} aria-label={t('vault.conflict.modeUnprotect')} />
            <span>{t('vault.conflict.modeUnprotect')}</span>
          </label>
          {mode === 'unprotect' && <p className="text-xs text-gray-400 mt-1 ml-6">{t('vault.conflict.unprotectFolderHint')}</p>}
        </fieldset>
        <p className="text-xs rounded border px-2 py-1.5 mb-3" style={{ borderColor: '#d97706', background: '#fffbeb', color: '#7c2d12' }}>
          {t('vault.conflict.warning')}
        </p>
        {error && <div className="text-sm text-red-400 mb-2" role="alert">{error}</div>}
        <div className="flex justify-end gap-2 mt-2">
          <button onClick={cancel} disabled={busy} className="px-3 py-1.5 rounded text-sm text-gray-300 hover:bg-gray-800 disabled:opacity-40">{t('vault.cancel')}</button>
          <button
            onClick={() => void submit()}
            disabled={!canSubmit}
            className="px-3 py-1.5 rounded text-sm font-medium disabled:opacity-40"
            style={{ background: 'var(--line)', color: '#1c1917' }}
          >
            {t('vault.conflict.submit')}
          </button>
        </div>
      </div>
    </div>
  );
}
