export interface WorkspaceInfo { id: string; name: string; role: string }
export type SyncState = 'local' | 'unbound' | 'syncing' | 'synced' | 'offline';
export interface SyncStatus {
  state: SyncState;
  lastSyncedAt: number;
  pending: number;
  /**
   * The workspace is waiting for its vault key to be rotated (a member was
   * removed). Carried here so the sidebar badge does not have to re-list every
   * context after every pull.
   */
  vaultRotationPending: boolean;
}
