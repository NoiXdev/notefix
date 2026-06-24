import { describe, it, expect, vi } from 'vitest';
vi.mock('./api', () => ({ api: { saveImage: vi.fn(() => Promise.resolve('noteimg://localhost/n/o/t/e/x.png')) } }));
import { saveImageFile } from './saveImage';
import { api } from './api';
describe('saveImageFile', () => {
  it('calls api.saveImage with the note id and a <uuid>.<ext> name', async () => {
    const file = new File([new Uint8Array([1, 2, 3])], 'pic.png', { type: 'image/png' });
    const url = await saveImageFile('note-1', file);
    expect(url).toBe('noteimg://localhost/n/o/t/e/x.png');
    const [noteId, name, bytes] = (api.saveImage as unknown as { mock: { calls: [string, string, number[]][] } }).mock.calls[0];
    expect(noteId).toBe('note-1');
    expect(name.endsWith('.png')).toBe(true);
    expect(bytes).toEqual([1, 2, 3]);
  });
});
