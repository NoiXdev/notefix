import { describe, it, expect, vi } from 'vitest';
vi.mock('./api', () => ({ api: { saveImage: vi.fn(() => Promise.resolve('noteimg://localhost/x.png')) } }));
import { saveImageFile } from './saveImage';
import { api } from './api';
describe('saveImageFile', () => {
  it('calls api.saveImage with a <uuid>.<ext> name and returns the url', async () => {
    const file = new File([new Uint8Array([1, 2, 3])], 'pic.png', { type: 'image/png' });
    const url = await saveImageFile(file);
    expect(url).toBe('noteimg://localhost/x.png');
    const [name, bytes] = (api.saveImage as unknown as { mock: { calls: [string, number[]][] } }).mock.calls[0];
    expect(name.endsWith('.png')).toBe(true);
    expect(bytes).toEqual([1, 2, 3]);
  });
});
