import { describe, it, expect, vi, beforeEach } from 'vitest';
import { runSystemChecks } from './systemChecks';
import { api } from './api';

vi.mock('./api', () => ({
  api: {
    checkPaths: vi.fn(),
    windowProbe: vi.fn(() => Promise.resolve(true)),
    autostart: { isEnabled: vi.fn(() => Promise.resolve(false)) },
  },
}));

const settings = { startMinimized: false } as never;

beforeEach(() => {
  (api.checkPaths as ReturnType<typeof vi.fn>).mockResolvedValue({ dbWritable: true, imagesWritable: true, dbPath: '/db', imagesPath: '/db/images' });
  (api.windowProbe as ReturnType<typeof vi.fn>).mockResolvedValue(true);
});

describe('runSystemChecks', () => {
  it('all ok when writable + window ok', async () => {
    const checks = await runSystemChecks(settings);
    expect(checks.find(c => c.key === 'db')!.status).toBe('ok');
    expect(checks.find(c => c.key === 'images')!.status).toBe('ok');
    expect(checks.find(c => c.key === 'window')!.status).toBe('ok');
    expect(checks.some(c => c.status === 'error')).toBe(false);
  });
  it('db not writable => error with changeLocation action', async () => {
    (api.checkPaths as ReturnType<typeof vi.fn>).mockResolvedValue({ dbWritable: false, imagesWritable: true, dbPath: '/db', imagesPath: '/db/images' });
    const checks = await runSystemChecks(settings);
    const db = checks.find(c => c.key === 'db')!;
    expect(db.status).toBe('error');
    expect(db.action).toBe('changeLocation');
  });
  it('window probe failing => error', async () => {
    (api.windowProbe as ReturnType<typeof vi.fn>).mockResolvedValue(false);
    const checks = await runSystemChecks(settings);
    expect(checks.find(c => c.key === 'window')!.status).toBe('error');
  });
});
