import { test, expect } from '@playwright/test';
import { installTauriMock } from './tauri-mock';

test('settings expose the copy-format select', async ({ page }) => {
  await installTauriMock(page);
  await page.goto('/');
  await page.getByTitle('Mehr').click();
  await page.getByText('Einstellungen').click();
  await page.getByText('Darstellung', { exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Kopier-Format' })).toBeVisible();
});
