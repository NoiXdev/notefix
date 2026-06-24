import { test, expect } from '@playwright/test';
import { installTauriMock } from './tauri-mock';

test('i18n: app starts in German (de-DE locale) and switches to English', async ({ page }) => {
  await installTauriMock(page);
  await page.goto('/');
  await page.getByTitle('Mehr').click();
  await page.getByText('Einstellungen').click();
  await page.getByText('Darstellung', { exact: true }).click();

  // Language select shows the German auto label; open it and choose English.
  await page.locator('div.max-w-sm', { hasText: 'Automatisch (System)' }).click();
  await page.getByText('English', { exact: true }).click();

  // UI is now English ("Darstellung" → "Appearance").
  await expect(page.getByText('Appearance').first()).toBeVisible();
});
