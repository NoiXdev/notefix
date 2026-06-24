import { test, expect } from '@playwright/test';
import { installTauriMock } from './tauri-mock';

test('export: settings export button opens the image-mode dialog', async ({ page }) => {
  await installTauriMock(page);
  await page.goto('/');
  await page.getByTitle('Mehr').click();
  await page.getByText('Einstellungen').click();
  await page.getByText('System', { exact: true }).click();
  await page.getByText('Alle als JSON exportieren').click();
  await expect(page.getByText('Wie sollen Bilder im Export behandelt werden?')).toBeVisible();
  await expect(page.getByText(/base64/)).toBeVisible();
  await expect(page.getByText(/Bundle/)).toBeVisible();
});
