export interface ContextInfo {
  id: string;
  label: string;
  kind: "local" | "server";
  path: string;
  active: boolean;
}
