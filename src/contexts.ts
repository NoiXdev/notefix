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
