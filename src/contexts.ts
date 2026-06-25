export interface ContextInfo {
  id: string;
  label: string;
  kind: "local" | "server";
  path: string;
  /** Base URL of the backing server; empty for local contexts. */
  serverUrl: string;
  active: boolean;
}
