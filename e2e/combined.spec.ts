import { test, expect } from '@playwright/test';
import { installTauriMock } from './tauri-mock';

test('combined sidebar mode lists notes from all contexts with badges', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', e => errors.push(e.message));

  await installTauriMock(page, {
    settings: [['sidebarMode', 'combined']],
    responses: {
      notes_load_all: [
        { contextId: 'c1', contextLabel: 'Privat', kind: 'local', note: { id: 'n1', preview: 'Erste', tasksDone: 0, tasksTotal: 0, updatedAt: 5, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0, deletedAt: null } },
        { contextId: 'c2', contextLabel: 'Team', kind: 'server', note: { id: 'n2', preview: 'Zweite', tasksDone: 0, tasksTotal: 0, updatedAt: 9, pinned: false, archived: false, color: '', dueAt: null, folderId: null, position: 0, deletedAt: null } },
      ],
    },
  });
  await page.goto('/');

  await expect(page.getByText('Erste')).toBeVisible();
  await expect(page.getByText('Zweite')).toBeVisible();
  await expect(page.getByText('Team')).toBeVisible(); // server context badge
  expect(errors).toEqual([]);
});
