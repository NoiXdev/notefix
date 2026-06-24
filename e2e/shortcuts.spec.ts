import { test, expect } from '@playwright/test';
import { installTauriMock } from './tauri-mock';

test('shortcuts: record a new combo on the keyboard page', async ({ page }) => {
  await installTauriMock(page);
  await page.goto('/');
  await page.getByTitle('Mehr').click();
  await page.getByText('Einstellungen').click();
  await page.getByText('Tastatur', { exact: true }).click();
  await expect(page.getByText('Neue Notiz')).toBeVisible();

  // "Ändern" buttons are in action order; newNote is the 3rd (navPrev, navNext, newNote).
  await page.getByRole('button', { name: 'Ändern' }).nth(2).click();
  await expect(page.getByText('Taste drücken…')).toBeVisible();
  await page.keyboard.press('Meta+J');
  await expect(page.getByText('Mod + J')).toBeVisible();
});
