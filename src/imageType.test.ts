import { describe, it, expect } from 'vitest';
import { extFromType } from './imageType';
describe('extFromType', () => {
  it('maps known mimes, falls back to png', () => {
    expect(extFromType('image/png')).toBe('png');
    expect(extFromType('image/jpeg')).toBe('jpeg');
    expect(extFromType('image/svg+xml')).toBe('svg');
    expect(extFromType('application/x-weird')).toBe('png');
  });

  it('maps image/gif to gif', () => {
    expect(extFromType('image/gif')).toBe('gif');
  });

  it('maps image/webp to webp', () => {
    expect(extFromType('image/webp')).toBe('webp');
  });

  it('falls back to png for an empty mime', () => {
    expect(extFromType('')).toBe('png');
  });
});
