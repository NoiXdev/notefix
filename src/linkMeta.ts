import { api } from './api';

export interface LinkMeta { url: string; title: string; description: string; image: string; site: string; }

const cache = new Map<string, Promise<LinkMeta>>();

export function fetchMeta(url: string): Promise<LinkMeta> {
  let p = cache.get(url);
  if (!p) {
    p = api.fetchLinkMeta(url).catch(() => ({ url, title: '', description: '', image: '', site: '' }));
    cache.set(url, p);
  }
  return p;
}

export function isBareUrl(text: string): boolean {
  const t = text.trim();
  return /^https?:\/\/[^\s]+$/.test(t);
}

/** test-only */
export function _resetCache(): void { cache.clear(); }
