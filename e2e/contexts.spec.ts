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
