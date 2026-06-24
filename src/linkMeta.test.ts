import { describe, it, expect, vi, beforeEach } from 'vitest';
vi.mock('./api', () => ({ api: { fetchLinkMeta: vi.fn(() => Promise.resolve({ url: 'u', title: 'T', description: '', image: '', site: 's' })) } }));
import { isBareUrl, fetchMeta, _resetCache } from './linkMeta';
import { api } from './api';

beforeEach(() => { _resetCache(); (api.fetchLinkMeta as ReturnType<typeof vi.fn>).mockClear(); });

describe('isBareUrl', () => {
  it('accepts a lone http(s) url, rejects text with spaces', () => {
    expect(isBareUrl('https://example.com/a')).toBe(true);
    expect(isBareUrl('  http://x.io  ')).toBe(true);
    expect(isBareUrl('see https://x.io')).toBe(false);
    expect(isBareUrl('not a url')).toBe(false);
    expect(isBareUrl('ftp://x')).toBe(false);
  });
});

describe('fetchMeta', () => {
  it('dedupes the same url to one api call', async () => {
    const a = fetchMeta('https://x.io');
    const b = fetchMeta('https://x.io');
    await Promise.all([a, b]);
    expect(api.fetchLinkMeta).toHaveBeenCalledTimes(1);
  });
});
