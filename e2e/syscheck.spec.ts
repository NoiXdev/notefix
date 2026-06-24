import { test, expect } from '@playwright/test';
import { installTauriMock } from './tauri-mock';

test('system check: startup modal appears when a folder is not writable', async ({ page }) => {
  await installTauriMock(page, { responses: { check_paths: { dbWritable: false, imagesWritable: true, dbPath: '/x', imagesPath: '/x/images' } } });
  await page.goto('/');
  await expect(page.getByText('Systemprüfung')).toBeVisible();
  await page.getByText('Einstellungen öffnen').click();
  await expect(page.getByRole('heading', { name: 'Diagnose' })).toBeVisible();
});

test('system check: no modal when everything is writable', async ({ page }) => {
  await installTauriMock(page);
  await page.goto('/');
  await expect(page.getByText('Notefix')).toBeVisible();
  await expect(page.getByText('Systemprüfung')).toHaveCount(0);
});
