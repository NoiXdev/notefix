import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

interface Props {
  biometricAvailable: boolean;
  /**
   * Whether this user holds a recovery key at all. On a workspace vault only
   * the members the server handed a recovery wrap to do — an invited member
   * gets a wrapped key and nothing else — so offering them the recovery field
   * would be a dead end. Defaults to `true`: a local vault's recovery key was
   * minted on this device.
   */
  recoveryAvailable?: boolean;
  unlock: (passphrase: string) => Promise<void>;
  unlockRecovery: (recovery: string) => Promise<void>;
  unlockBiometric: () => Promise<void>;
  /**
   * Asked once a passphrase unlock succeeded: is the workspace waiting for
   * this member to redeem a one-time rotation code? Only then does the dialog
   * show its rotation step. Left out (with `redeemRotation`) by callers that
   * do not care — the dialog then closes on a successful unlock as before.
   */
  rotationPending?: () => Promise<boolean>;
  redeemRotation?: (code: string, passphrase: string) => Promise<void>;
  /**
   * `needsRotationCode` is true only for a path that unlocked WITHOUT typing a
   * passphrase — Touch ID — while a rotation code is still waiting. That step
   * cannot happen in here (it would have to ask for the passphrase the user
   * just skipped), so the caller takes it over.
   */
  onSuccess: (needsRotationCode?: boolean) => void;
  onCancel: () => void;
}

/**
 * In-app "unlock the vault" dialog: Touch ID (if available), passphrase, or
 * a recovery key as a fallback.
 *
 * After a passphrase unlock it may ask for one more thing: when the workspace
 * key was rotated, the member's new key is parked under a one-time rotation
 * code, and the passphrase they just proved is exactly what the redemption
 * re-wraps it under — so the step belongs right here rather than in a
 * separate dialog that would ask for the passphrase a second time.
 */
export default function VaultUnlock({ biometricAvailable, recoveryAvailable = true, unlock, unlockRecovery, unlockBiometric, rotationPending, redeemRotation, onSuccess, onCancel }: Props) {
  const { t } = useTranslation();
  const [mode, setMode] = useState<'passphrase' | 'recovery' | 'rotation'>('passphrase');
  const [passphrase, setPassphrase] = useState('');
  const [recovery, setRecovery] = useState('');
  const [code, setCode] = useState('');
  const [error, setError] = useState<string | null>(null);
  const autoTried = useRef(false);

  const submitBiometric = async () => {
    setError(null);
    try {
      await unlockBiometric();
    } catch (e) {
      // The backend refuses a keychain DEK that doesn't belong to this
      // context's vault, and one whose vault predates the ownership check;
      // both need a different action than "try Touch ID again".
      const msg = e instanceof Error ? e.message : String(e ?? '');
      if (msg.includes('different context')) setError(t('vault.biometricOtherContext'));
      else if (msg.includes('upgrading this vault')) setError(t('vault.biometricNeedsPassphrase'));
      else setError(t('vault.biometricFailed'));
      return;
    }
    // Asked OUTSIDE the try above: the unlock has already succeeded, and a
    // failing "is a rotation waiting?" query must not be reported as a failed
    // Touch ID unlock. Unknown simply means "no extra step for now".
    let pending = false;
    try {
      pending = rotationPending ? await rotationPending() : false;
    } catch {
      pending = false;
    }
    // Touch ID types no passphrase, so the in-dialog rotation step cannot
    // re-wrap anything — the caller shows the standalone prompt instead.
    onSuccess(pending);
  };

  // Auto-trigger Touch ID once when the dialog opens, so the common case
  // (macOS, biometric enrolled) needs no click. Guarded by a ref so a
  // rejection (cancel / failed scan) doesn't loop — the button stays as a
  // fallback to retry or switch to the passphrase.
  useEffect(() => {
    if (!biometricAvailable || autoTried.current) return;
    autoTried.current = true;
    void submitBiometric();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [biometricAvailable]);

  const submitPassphrase = async () => {
    setError(null);
    try {
      await unlock(passphrase);
    } catch {
      setError(t('vault.wrongPassphrase'));
      return;
    }
    // The key was rotated while this member was away: their new key is
    // waiting under a one-time code, and re-wrapping it needs the passphrase
    // they just typed.
    if (redeemRotation && rotationPending && (await rotationPending())) {
      setMode('rotation');
      return;
    }
    onSuccess();
  };

  const submitRecovery = async () => {
    setError(null);
    try {
      await unlockRecovery(recovery);
      onSuccess();
    } catch {
      setError(t('vault.wrongPassphrase'));
    }
  };

  const submitRotation = async () => {
    if (!redeemRotation) return;
    setError(null);
    try {
      await redeemRotation(code, passphrase);
      onSuccess();
    } catch {
      setError(t('vault.rotation.invalidCode'));
    }
  };

  const submit = () => {
    if (mode === 'passphrase') return submitPassphrase();
    if (mode === 'recovery') return submitRecovery();
    return submitRotation();
  };

  const switchMode = (next: 'passphrase' | 'recovery') => {
    setMode(next);
    setError(null);
  };

  /**
   * Dismissing the dialog. In the rotation step the vault is ALREADY unlocked
   * — the code only fetches the newest key generation — so backing out is a
   * success, not a cancel: reporting it as a cancel would throw away a
   * pending protect the user started before ever seeing this step.
   */
  const dismiss = () => (mode === 'rotation' ? onSuccess() : onCancel());

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center" style={{ background: 'rgba(0,0,0,0.45)' }} onClick={dismiss}>
      <div className="w-96 rounded-lg bg-gray-900 border border-gray-700 p-5" onClick={e => e.stopPropagation()}>
        <h2 className="text-gray-100 text-base font-semibold mb-3">
          {mode === 'rotation' ? t('vault.rotation.enterCode') : t('vault.unlockTitle')}
        </h2>
        {mode === 'rotation' && <p className="text-gray-400 text-sm mb-3">{t('vault.rotation.enterCodeHint')}</p>}

        {biometricAvailable && mode === 'passphrase' && (
          <button
            onClick={() => void submitBiometric()}
            className="w-full mb-3 px-3 py-2 rounded text-sm font-medium"
            style={{ background: 'var(--line)', color: '#1c1917' }}
          >
            {t('vault.unlockTouchId')}
          </button>
        )}

        {mode === 'passphrase' && (
          <input
            type="password"
            autoFocus
            value={passphrase}
            placeholder={t('vault.passphrase')}
            onChange={e => setPassphrase(e.target.value)}
            onKeyDown={e => { if (e.key === 'Enter') void submitPassphrase(); else if (e.key === 'Escape') dismiss(); }}
            className="w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-gray-100 outline-none focus:border-[var(--accent)] mb-2"
          />
        )}
        {mode === 'recovery' && (
          <input
            autoFocus
            value={recovery}
            placeholder={t('vault.recoveryKey')}
            onChange={e => setRecovery(e.target.value)}
            onKeyDown={e => { if (e.key === 'Enter') void submitRecovery(); else if (e.key === 'Escape') dismiss(); }}
            className="w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-gray-100 outline-none focus:border-[var(--accent)] mb-2 font-mono"
          />
        )}
        {mode === 'rotation' && (
          <input
            autoFocus
            value={code}
            placeholder={t('vault.rotation.code')}
            onChange={e => setCode(e.target.value)}
            onKeyDown={e => { if (e.key === 'Enter') void submitRotation(); else if (e.key === 'Escape') dismiss(); }}
            className="w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-gray-100 outline-none focus:border-[var(--accent)] mb-2 font-mono"
          />
        )}

        {error && (
          <div className="text-sm text-red-400 mb-2" role="alert">
            {error}
          </div>
        )}

        {recoveryAvailable && mode !== 'rotation' && (
          <button
            onClick={() => switchMode(mode === 'passphrase' ? 'recovery' : 'passphrase')}
            className="text-xs text-gray-400 hover:text-gray-200 underline mb-3"
          >
            {mode === 'passphrase' ? t('vault.useRecovery') : t('vault.usePassphrase')}
          </button>
        )}

        <div className="flex justify-end gap-2">
          <button
            onClick={dismiss}
            className="px-3 py-1.5 rounded text-sm text-gray-300 hover:bg-gray-800"
          >
            {mode === 'rotation' ? t('vault.rotation.later') : t('vault.cancel')}
          </button>
          <button
            onClick={() => void submit()}
            className="px-3 py-1.5 rounded text-sm font-medium"
            style={{ background: 'var(--line)', color: '#1c1917' }}
          >
            {mode === 'rotation' ? t('vault.rotation.submit') : t('vault.unlock')}
          </button>
        </div>
      </div>
    </div>
  );
}
