import { describe, it, expect } from 'vitest';
import { parseLayout } from './useSettings';

describe('parseLayout', () => {
  it('migrates old string[] to {key,w:1}[]', () => {
    expect(parseLayout(JSON.stringify(['recent', 'stats']))).toEqual([{ key: 'recent', w: 1 }, { key: 'stats', w: 1 }]);
  });
  it('keeps the new {key,w}[] format', () => {
    const v = [{ key: 'stats', w: 2 }];
    expect(parseLayout(JSON.stringify(v))).toEqual(v);
  });
  it('falls back to default on garbage', () => {
    expect(parseLayout('nope').length).toBeGreaterThan(0);
    expect(parseLayout(JSON.stringify([{ key: 'x', w: 5 }])).length).toBeGreaterThan(0);
  });
});
