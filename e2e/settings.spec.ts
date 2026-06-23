import { test, expect, type Page } from '@playwright/test';
import { installTauriMock } from './tauri-mock';

async function openSettings(page: Page): Promise<void> {
  await page.goto('/');
  await page.getByTitle('Mehr').click();
  await page.getByText('Einstellungen').click();
}

test('settings: navigating every page throws no errors', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', e => errors.push(e.message));

  await installTauriMock(page);
  await openSettings(page);
  for (const nav of ['Darstellung', 'System', 'Statistik', 'Tastatur', 'About']) {
    await page.getByText(nav, { exact: true }).click();
  }
  expect(errors).toEqual([]);
});

test('settings/system: close-behavior select is in the viewport (not below the fold)', async ({ page }) => {
  await installTauriMock(page);
  await openSettings(page);
  await page.getByText('System', { exact: true }).click();

  const field = page.getByText('Beim Schließen des Fensters');
  await expect(field).toBeVisible();
  // Regression guard: the field used to render below the fold of the inner
  // scroll panel, so it was unreachable without a (hidden) scroll on macOS.
  await expect(field).toBeInViewport();
  await expect(page.getByText('Fragen')).toBeVisible();
});

test('the close prompt shows on top of Settings, not hidden behind it', async ({ page }) => {
  await installTauriMock(page);
  await openSettings(page);
  await expect(page.getByText('Settings')).toBeVisible(); // settings panel is open

  // The window "close-requested" event can fire while Settings is open. The
  // dialog used to render after an early `return <Settings/>`, so it only
  // appeared once Settings was closed. It must now overlay Settings instead.
  await page.evaluate(() => (window as unknown as { __emitTauriEvent: (e: string) => void }).__emitTauriEvent('close-requested'));

  await expect(page.getByText('Notefix schließen')).toBeVisible();
  await expect(page.getByText('Settings')).toBeVisible(); // Settings still mounted underneath
});

test('settings/system: close-behavior dropdown opens and lists its options', async ({ page }) => {
  await installTauriMock(page);
  await openSettings(page);
  await page.getByText('System', { exact: true }).click();

  // Open the react-select control (currently showing "Fragen"). This exercises
  // real react-select in a browser — behaviour jsdom unit mocks cannot verify.
  await expect(page.getByText('Fragen')).toBeVisible();
  await page.locator('label', { hasText: 'Beim Schließen des Fensters' }).locator('div.w-56').click();
  await expect(page.getByText('Beenden')).toBeVisible();
  await expect(page.getByText('In Menüleiste minimieren')).toBeVisible();
});
