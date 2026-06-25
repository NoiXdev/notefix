import { test, expect } from '@playwright/test';
import { installTauriMock } from './tauri-mock';

test('context switcher mounts and its menu exposes "Verwalten…"', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', e => errors.push(e.message));

  await installTauriMock(page);
  await page.goto('/');

  // The switcher lives in the sidebar header and is labelled in German under de-DE.
  const switcher = page.getByLabel('Kontext wechseln');
  await expect(switcher).toBeVisible();

  // Opening it must not throw and must surface the manage entry.
  await switcher.click();
  await expect(page.getByText('Verwalten…')).toBeVisible();

  expect(errors).toEqual([]);
});

test('add-server flow: dialog → begin → auth-callback completes', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', e => errors.push(e.message));

  await installTauriMock(page);
  await page.goto('/');

  // Open the switcher and start adding a server.
  await page.getByLabel('Kontext wechseln').click();
  await page.getByText('Server hinzufügen…').click();

  // Enter the server URL and confirm.
  await page.getByPlaceholder('Server-URL (z. B. https://notes.example.com)').fill('https://srv.example');
  await page.getByRole('button', { name: 'Server hinzufügen…' }).click();

  // The discovery + browser-open ran (server_auth_begin) and we show "Verbinde…".
  await expect(page.getByText('Verbinde…')).toBeVisible();
  await expect.poll(() => page.evaluate(() => (window as unknown as { __tauriCalls: string[] }).__tauriCalls))
    .toContain('server_auth_begin');

  // Simulate the browser redirect (notefix://auth?code=…&state=…) arriving.
  await page.evaluate(() => (window as unknown as { __emitTauriEvent: (e: string, p?: unknown) => void })
    .__emitTauriEvent('auth-callback', 'notefix://auth?code=abc&state=x'));
  await expect.poll(() => page.evaluate(() => (window as unknown as { __tauriCalls: string[] }).__tauriCalls))
    .toContain('server_auth_complete');

  // The server's context-changed broadcast clears the pending indicator.
  await page.evaluate(() => (window as unknown as { __emitTauriEvent: (e: string, p?: unknown) => void })
    .__emitTauriEvent('context-changed'));
  await expect(page.getByText('Verbinde…')).toBeHidden();

  expect(errors).toEqual([]);
});

test('unbound server context opens the workspace picker and binds', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', e => errors.push(e.message));

  await installTauriMock(page, {
    responses: {
      contexts_list: [{ id: 'srv', label: '', kind: 'server', path: '/s.db', serverUrl: 'https://s', workspaceId: '', active: true }],
    },
  });
  await page.goto('/');

  // App checks the active context on load; an unbound server context opens the picker.
  await expect(page.getByText('Workspace wählen')).toBeVisible();
  await page.getByText('Privat').click();

  await expect.poll(() => page.evaluate(() => (window as unknown as { __tauriCalls: string[] }).__tauriCalls))
    .toContain('context_bind_workspace');
  expect(errors).toEqual([]);
});
