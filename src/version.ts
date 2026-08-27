import type { ReleaseInfo } from './api';

/** Digits-only (after stripping an optional leading `v`) — everything else
 * ("latest", "nightly", "", …) is not a version we can order. */
const VERSION_LIKE = /^v?\d/i;

/**
 * Parses a version string ("0.6.0" or "v0.6.0") into [major, minor, patch].
 * Tolerant of a leading `v` and of missing/non-numeric components, which are
 * treated as 0 (so "0.6" -> [0, 6, 0], "garbage" -> [0, 0, 0]).
 */
export function parseVersion(v: string): [number, number, number] {
  const parts = v.trim().replace(/^v/i, '').split('.');
  const num = (i: number): number => {
    const n = Number.parseInt(parts[i] ?? '', 10);
    return Number.isFinite(n) ? n : 0;
  };
  return [num(0), num(1), num(2)];
}

/** Component-wise version compare: negative if a<b, 0 if equal, positive if a>b. */
export function compareVersions(a: string, b: string): number {
  const pa = parseVersion(a);
  const pb = parseVersion(b);
  for (let i = 0; i < 3; i++) {
    if (pa[i] !== pb[i]) return pa[i] - pb[i];
  }
  return 0;
}

/** True when `a` is a strictly newer version than `b`. */
export function isNewer(a: string, b: string): boolean {
  return compareVersions(a, b) > 0;
}

/**
 * The releases strictly newer than `lastSeen` and at-or-below `current`,
 * newest first — the cumulative "what's new since you last looked" set.
 * Releases whose tag doesn't look like a version (e.g. "latest") are ignored.
 */
export function releasesSince(releases: ReleaseInfo[], lastSeen: string, current: string): ReleaseInfo[] {
  return releases
    .filter(r => VERSION_LIKE.test(r.tagName.trim()))
    .filter(r => isNewer(r.tagName, lastSeen) && compareVersions(r.tagName, current) <= 0)
    .sort((a, b) => compareVersions(b.tagName, a.tagName));
}
