import { test, expect } from '@playwright/test';
import { installTauriMock } from './tauri-mock';

test('dashboard: opens from the ⋯ menu and shows default widgets', async ({ page }) => {
  await installTauriMock(page);
  await page.goto('/');
  await page.getByTitle('Mehr').click();
  await page.getByText('Dashboard', { exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Dashboard' })).toBeVisible();
  await expect(page.getByText('Zuletzt bearbeitet')).toBeVisible();
});

test('dashboard: edit mode exposes handles + add buttons and can add a widget', async ({ page }) => {
  await installTauriMock(page);
  await page.goto('/');
  await page.getByTitle('Mehr').click();
  await page.getByText('Dashboard', { exact: true }).click();
  await page.getByText('Bearbeiten').click();
  await expect(page.getByText('Fertig')).toBeVisible();
  // available widgets (not in the default layout) appear as add buttons
  await expect(page.getByText('+ Schnellnotiz')).toBeVisible();
  await page.getByText('+ Schnellnotiz').click();
  // the added widget renders as a card title
  await expect(page.getByText('Schnellnotiz', { exact: true })).toBeVisible();
});
