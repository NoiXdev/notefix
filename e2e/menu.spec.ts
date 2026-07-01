import { test, expect } from '@playwright/test';
import { installTauriMock } from './tauri-mock';
const oneNote = { notes: [{ id: 'a', preview: 'Hallo', tasksDone: 0, tasksTotal: 0, updatedAt: 1, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0, deletedAt: null }] };
test('overflow menu opens and an item switches view', async ({ page }) => {
  await installTauriMock(page, oneNote);
  await page.goto('/');
  await page.getByTitle('Mehr').click();
  await expect(page.getByText('Einstellungen')).toBeVisible();
  await page.getByText('Papierkorb').click();
  await expect(page.getByText('Papierkorb ist leer.')).toBeVisible();
});
test('a folder submenu opens on click', async ({ page }) => {
  await installTauriMock(page, { folders: [{ id: 'f1', name: 'Arbeit', parentId: null, position: 1, icon: '', color: '', sort: 'manual' }] });
  await page.goto('/');
  await page.getByText('Arbeit').click({ button: 'right' });
  await page.getByText('Sortierung').click();
  await expect(page.getByText('Titel A–Z')).toBeVisible();
});
test('opening a new menu closes the previous one', async ({ page }) => {
  await installTauriMock(page, oneNote);
  await page.goto('/');
  await page.locator('aside').getByText('Hallo').click({ button: 'right' });
  await expect(page.getByText('Anpinnen')).toBeVisible();
  await page.locator('aside').click({ button: 'right', position: { x: 10, y: 250 } });
  await expect(page.getByText('Anpinnen')).toHaveCount(0);
});
