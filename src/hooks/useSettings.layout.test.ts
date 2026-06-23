import { describe, it, expect } from 'vitest';
import { parseLayout } from './useSettings';

describe('parseLayout', () => {
  it('migrates old {key,w:1|2}[] to grid items', () => {
    const r = parseLayout(JSON.stringify([{ key: 'recent', w: 1 }, { key: 'stats', w: 2 }]));
    expect(r).toEqual([
      { key: 'recent', x: 0, y: 0, w: 6, h: 4 },
      { key: 'stats', x: 0, y: 4, w: 12, h: 4 },
    ]);
  });
  it('migrates legacy string[] to grid items', () => {
    expect(parseLayout(JSON.stringify(['recent']))).toEqual([{ key: 'recent', x: 0, y: 0, w: 6, h: 4 }]);
  });
  it('keeps the new {key,x,y,w,h}[] format', () => {
    const v = [{ key: 'stats', x: 2, y: 3, w: 4, h: 3 }];
    expect(parseLayout(JSON.stringify(v))).toEqual(v);
  });
  it('falls back to default on garbage', () => {
    expect(parseLayout('nope').length).toBeGreaterThan(0);
    expect(parseLayout(JSON.stringify([{ key: 'x', w: 5 }])).length).toBeGreaterThan(0);
  });
});
