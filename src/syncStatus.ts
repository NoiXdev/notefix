export interface WorkspaceInfo { id: string; name: string; role: string }
export type SyncState = 'local' | 'unbound' | 'syncing' | 'synced' | 'offline';
export interface SyncStatus { state: SyncState; lastSyncedAt: number; pending: number }
