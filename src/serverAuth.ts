import { api } from './api';

/**
 * Start the browser sign-in flow for a server context. Discovers the server's
 * OAuth config, then opens the system browser at the authorize URL. The flow
 * finishes asynchronously when the `notefix://auth` redirect fires the
 * `auth-callback` event (handled centrally in App via api.onAuthCallback →
 * api.contexts.serverAuthComplete).
 *
 * Rejects when discovery fails (unreachable server, bad URL, no OAuth config),
 * so callers can surface that to the user before the browser ever opens.
 */
export async function startServerAuth(serverUrl: string): Promise<void> {
  const authorizeUrl = await api.contexts.serverAuthBegin(serverUrl);
  await api.openExternal(authorizeUrl);
}
