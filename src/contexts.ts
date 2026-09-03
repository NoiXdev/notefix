export interface ContextInfo {
  id: string;
  label: string;
  kind: "local" | "server";
  path: string;
  /** Base URL of the backing server; empty for local contexts. */
  serverUrl: string;
  /** Bound server workspace id; empty until a workspace is picked. */
  workspaceId: string;
  active: boolean;
  /** Whether this context's own DB has a vault record set up. */
  vaultExists: boolean;
  /** Whether this context has a Touch ID keychain item enrolled. */
  vaultBiometric: boolean;
  /** Newest workspace key generation this context has pulled; 0 when local. */
  vaultGeneration: number;
  /** Whether the workspace still owes this context's vault a key rotation. */
  vaultRotationPending: boolean;
  /** The user's role in the workspace as of the last pull; "" for local contexts. */
  role: string;
  /** Open invitations whose vault code was lost in a rotation (owners only). */
  invitesNeedingCode: number;
}

/**
 * The context to switch to when cycling forward from the active one (wraps
 * around at the end). Returns null when there is nothing to switch to (fewer
 * than two contexts). If none is marked active, starts from the first.
 */
export function nextContextId(contexts: ContextInfo[]): string | null {
  if (contexts.length < 2) return null;
  const idx = contexts.findIndex(c => c.active);
  const next = contexts[(Math.max(0, idx) + 1) % contexts.length];
  return next && !next.active ? next.id : null;
}
