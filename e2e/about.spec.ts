import { test, expect } from '@playwright/test';
import { installTauriMock } from './tauri-mock';

test('about page shows the MIT license + project link', async ({ page }) => {
  await installTauriMock(page);
  await page.goto('/');
  await page.getByTitle('Mehr').click();
  await page.getByText('Einstellungen').click();
  // About is the default settings page.
  await expect(page.getByText('MIT License')).toBeVisible();
  await expect(page.getByText('Projekt: noix.dev')).toBeVisible();
  // Open-source acknowledgements list.
  await expect(page.getByRole('heading', { name: 'Open Source' })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Tiptap' })).toBeVisible();
  await expect(page.getByRole('link', { name: 'highlight.js' })).toBeVisible();
});
