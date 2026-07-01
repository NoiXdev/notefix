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
