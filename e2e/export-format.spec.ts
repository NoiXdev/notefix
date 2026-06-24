import { test, expect } from '@playwright/test';
import { installTauriMock } from './tauri-mock';

const note = { id: 'n1', content: '<p>Hallo Welt</p>', updatedAt: 1, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0, deletedAt: null };

test('note context menu → export opens the format modal', async ({ page }) => {
  await installTauriMock(page, { notes: [note] });
  await page.goto('/');
  await page.getByText('Hallo Welt').click({ button: 'right' });
  await page.getByText('Exportieren').click();
  await expect(page.getByText('Notiz exportieren')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Markdown' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'PDF' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Word' })).toBeVisible();
});
