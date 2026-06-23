import { test, expect } from '@playwright/test';
import { installTauriMock } from './tauri-mock';

test('app loads and shows the sidebar', async ({ page }) => {
  await installTauriMock(page);
  await page.goto('/');
  await expect(page.getByText('Notefix')).toBeVisible();
  await expect(page.getByTitle('New note')).toBeVisible();
});

test('creating a note opens the editor', async ({ page }) => {
  await installTauriMock(page);
  await page.goto('/');
  await page.getByTitle('New note').click();
  await expect(page.getByTitle('Bold')).toBeVisible();
});
