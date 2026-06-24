import { test, expect } from '@playwright/test';
import { installTauriMock } from './tauri-mock';

test('app loads and shows the sidebar', async ({ page }) => {
  await installTauriMock(page);
  await page.goto('/');
  await expect(page.getByText('Notefix')).toBeVisible();
  await expect(page.getByTitle('Neue Notiz')).toBeVisible();
});

test('creating a note opens the editor', async ({ page }) => {
  await installTauriMock(page);
  await page.goto('/');
  await page.getByTitle('Neue Notiz').click();
  await expect(page.getByTitle('Fett')).toBeVisible();
});
