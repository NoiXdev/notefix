import { describe, it, expect } from 'vitest';
import { parseVersion, compareVersions, isNewer, releasesSince } from './version';
import type { ReleaseInfo } from './api';

describe('parseVersion', () => {
  it('parses a plain version', () => {
    expect(parseVersion('0.6.0')).toEqual([0, 6, 0]);
  });

  it('tolerates a leading v', () => {
    expect(parseVersion('v0.6.0')).toEqual([0, 6, 0]);
    expect(parseVersion('V1.2.3')).toEqual([1, 2, 3]);
  });

  it('defaults missing components to 0', () => {
    expect(parseVersion('0.6')).toEqual([0, 6, 0]);
    expect(parseVersion('1')).toEqual([1, 0, 0]);
  });

  it('treats non-numeric components as 0', () => {
    expect(parseVersion('garbage')).toEqual([0, 0, 0]);
    expect(parseVersion('')).toEqual([0, 0, 0]);
  });
});

describe('compareVersions', () => {
  it('returns 0 for equal versions (with or without a leading v)', () => {
    expect(compareVersions('0.6.0', '0.6.0')).toBe(0);
    expect(compareVersions('0.6.0', 'v0.6.0')).toBe(0);
  });

  it('orders by major, then minor, then patch', () => {
    expect(compareVersions('1.0.0', '0.9.9')).toBeGreaterThan(0);
    expect(compareVersions('0.6.1', '0.6.0')).toBeGreaterThan(0);
    expect(compareVersions('0.5.9', '0.6.0')).toBeLessThan(0);
  });
});

describe('isNewer', () => {
  it('detects newer versions', () => {
    expect(isNewer('0.6.1', '0.6.0')).toBe(true);
    expect(isNewer('v1.0.0', '0.9.0')).toBe(true);
  });

  it('is false for equal or older versions', () => {
    expect(isNewer('0.6.0', '0.6.0')).toBe(false);
    expect(isNewer('0.5.0', '0.6.0')).toBe(false);
  });
});

function release(tagName: string, extra: Partial<ReleaseInfo> = {}): ReleaseInfo {
  return { tagName, name: tagName, body: `notes for ${tagName}`, publishedAt: '2026-01-01T00:00:00Z', prerelease: false, ...extra };
}

describe('releasesSince', () => {
  it('returns releases strictly newer than lastSeen and up to current, newest first', () => {
    const releases = [release('v0.6.0'), release('v0.5.1'), release('v0.5.0'), release('v0.4.0')];
    const result = releasesSince(releases, '0.4.0', '0.6.0');
    expect(result.map(r => r.tagName)).toEqual(['v0.6.0', 'v0.5.1', 'v0.5.0']);
  });

  it('covers a multi-version gap cumulatively', () => {
    const releases = [release('2.0.0'), release('1.5.0'), release('1.2.0'), release('1.0.0'), release('0.9.0')];
    const result = releasesSince(releases, '1.0.0', '1.5.0');
    expect(result.map(r => r.tagName)).toEqual(['1.5.0', '1.2.0']);
  });

  it('excludes releases newer than current', () => {
    const releases = [release('0.7.0'), release('0.6.0')];
    const result = releasesSince(releases, '0.5.0', '0.6.0');
    expect(result.map(r => r.tagName)).toEqual(['0.6.0']);
  });

  it('excludes lastSeen itself (strictly greater only)', () => {
    const releases = [release('0.6.0'), release('0.5.0')];
    const result = releasesSince(releases, '0.6.0', '0.6.0');
    expect(result).toEqual([]);
  });

  it('ignores releases whose tag does not parse as a version', () => {
    const releases = [release('v0.6.0'), release('latest'), release('nightly-build')];
    const result = releasesSince(releases, '0.5.0', '0.6.0');
    expect(result.map(r => r.tagName)).toEqual(['v0.6.0']);
  });

  it('sorts unordered input newest-first', () => {
    const releases = [release('0.5.0'), release('0.6.0'), release('0.5.5')];
    const result = releasesSince(releases, '0.4.0', '0.6.0');
    expect(result.map(r => r.tagName)).toEqual(['0.6.0', '0.5.5', '0.5.0']);
  });

  it('returns an empty array when there is nothing new', () => {
    const releases = [release('0.5.0')];
    expect(releasesSince(releases, '0.6.0', '0.6.0')).toEqual([]);
  });
});
