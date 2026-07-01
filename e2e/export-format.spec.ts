import { test, expect } from '@playwright/test';
import { installTauriMock } from './tauri-mock';

const note = { id: 'n1', preview: 'Hallo Welt', tasksDone: 0, tasksTotal: 0, updatedAt: 1, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0, deletedAt: null };

test('note context menu → export opens the format modal', async ({ page }) => {
  await installTauriMock(page, { notes: [note] });
  await page.goto('/');
  // "Hallo Welt" appears in both the note row and the auto-opened editor;
  // target the note row (first match) to right-click its context menu.
  const row = page.getByText('Hallo Welt').first();
  await expect(row).toBeVisible();
  await row.click({ button: 'right' });
  await page.getByText('Exportieren').click();
  await expect(page.getByText('Notiz exportieren')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Markdown' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'PDF' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Word' })).toBeVisible();
});
