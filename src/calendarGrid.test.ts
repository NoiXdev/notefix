import { describe, it, expect } from 'vitest';
import { monthGrid } from './calendarGrid';

describe('monthGrid', () => {
  it('January 2026 (1st is a Thursday, Mon-based lead=3)', () => {
    const g = monthGrid(2026, 0);
    expect(g[0].slice(0, 4)).toEqual([null, null, null, 1]);
    expect(g.flat().filter(d => d !== null)).toHaveLength(31);
    expect(g.every(w => w.length === 7)).toBe(true);
  });
  it('February 2024 is a leap month (29 days)', () => {
    expect(monthGrid(2024, 1).flat().filter(d => d !== null)).toHaveLength(29);
  });
});
