import type { UpdateInfo } from './api';

/**
 * Whether the startup update banner should appear: an update is available and
 * the user hasn't already dismissed the banner for exactly that version.
 */
export function shouldShowUpdateBanner(info: UpdateInfo | null, dismissedVersion: string): boolean {
  if (!info || !info.updateAvailable) return false;
  return info.latest !== dismissedVersion;
}
