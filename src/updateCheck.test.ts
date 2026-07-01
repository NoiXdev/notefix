import { describe, it, expect } from 'vitest';
import { shouldShowUpdateBanner } from './updateCheck';
import type { UpdateInfo } from './api';

const info = (over: Partial<UpdateInfo> = {}): UpdateInfo => ({
  current: '0.1.2', latest: '0.1.3', updateAvailable: true, url: 'https://x', ...over,
});

describe('shouldShowUpdateBanner', () => {
  it('shows when an update is available and not dismissed', () => {
    expect(shouldShowUpdateBanner(info(), '')).toBe(true);
    expect(shouldShowUpdateBanner(info(), '0.1.1')).toBe(true); // an older dismissal
  });

  it('hides when up to date or no info', () => {
    expect(shouldShowUpdateBanner(info({ updateAvailable: false }), '')).toBe(false);
    expect(shouldShowUpdateBanner(null, '')).toBe(false);
  });

  it('hides when the current latest was already dismissed', () => {
    expect(shouldShowUpdateBanner(info({ latest: '0.1.3' }), '0.1.3')).toBe(false);
  });
});
