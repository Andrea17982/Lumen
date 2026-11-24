// M5 against the real daemon: projects, conversations in the sidebar (new, search, rename, pin,
// reorder, move, delete), the project panel, and a list of 500 conversations.
import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { realpathSync } from 'node:fs';
import { mkdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { startDaemon } from './daemon-fixture';

test.describe.configure({ mode: 'serial' });

let daemon: Awaited<ReturnType<typeof startDaemon>>;
let other: string;
// The daemon keeps folders in their full form; temp paths can arrive in the short 8.3 form.
let otherReal: string;
let workspaceReal: string;
test.beforeAll(async ({ baseURL }) => {
  daemon = await startDaemon(new URL(baseURL!).origin);
  other = path.join(daemon.data, 'site');
  await mkdir(other);
  otherReal = realpathSync.native(other).toLowerCase();
  workspaceReal = realpathSync.native(daemon.workspace).toLowerCase();
});
test.afterAll(async () => daemon?.close());

type Row = { id: string; title: string; pinned: boolean; workspace: string; workspaceId: string | null };
const sessions = async () => (await daemon.client.call('list_sessions', { search: '', offset: 0 })) as Row[];
const nav = (page: Page) => page.getByRole('navigation', { name: 'Sessions' });
const rows = (page: Page) => nav(page).getByRole('listitem');

async function seriousViolations(page: Page) {
  const { violations } = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa', 'wcag22aa']).analyze();
  return violations.filter((v) => v.impact === 'serious' || v.impact === 'critical').map((v) => v.id);
}

test('a project is added from its folder and a new task starts in it', async ({ page }) => {
  await page.goto(daemon.url());
  await page.getByRole('button', { name: /^Project: Fixture/ }).click();
  page.once('dialog', (d) => d.accept(other));
  await page.getByRole('menuitem', { name: 'Add project…' }).click();
  await expect(page.getByRole('button', { name: /^Project: site/ })).toBeVisible();
  await expect(nav(page)).toContainText('No conversations yet');
  await page.getByRole('button', { name: 'New task' }).click();
  await expect(page.locator('main header h1')).toHaveText('New conversation');
  const created = (await sessions())[0];
  expect(created.workspace.toLowerCase()).toBe(otherReal);
  // Back to the first project: its own list, not this one.
  await page.getByRole('button', { name: /^Project: site/ }).click();
  await page.getByRole('menuitem', { name: 'Fixture' }).click();
  await expect(nav(page)).toContainText('No conversations yet');
  expect(await seriousViolations(page)).toEqual([]);
});

test('conversations are renamed, pinned, reordered, moved and deleted from the sidebar', async ({ page }) => {
  for (const _ of [1, 2, 3]) await daemon.client.call('create_session', { workspace: daemon.workspace });
  const [c, b, a] = (await sessions()).filter((x) => x.workspace.toLowerCase() === workspaceReal);
  await daemon.client.call('update_session', { id: a.id, title: 'Alpha', pinned: false });
  await daemon.client.call('update_session', { id: b.id, title: 'Beta', pinned: false });
  await daemon.client.call('update_session', { id: c.id, title: 'Gamma', pinned: false });
  await page.goto(daemon.url());
  await expect(rows(page)).toHaveText([/Gamma/, /Beta/, /Alpha/]);

  // Rename inline.
  await rows(page).filter({ hasText: 'Alpha' }).hover();
  await rows(page).filter({ hasText: 'Alpha' }).getByRole('button', { name: 'More actions' }).click();
  await page.getByRole('menuitem', { name: 'Rename' }).click();
  await nav(page).getByRole('textbox', { name: 'Title' }).fill('Alpha renamed');
  await nav(page).getByRole('textbox', { name: 'Title' }).press('Enter');
  await expect(rows(page).filter({ hasText: 'Alpha renamed' })).toHaveCount(1);
  expect((await sessions()).find((x) => x.id === a.id)?.title).toBe('Alpha renamed');

  // Pin: it moves to its own group, first.
  await rows(page).filter({ hasText: 'Beta' }).getByRole('button', { name: 'More actions' }).click();
  await page.getByRole('menuitem', { name: 'Pin' }).click();
  await expect(nav(page).getByRole('heading').first()).toHaveText('Pinned');
  await expect(rows(page).first()).toContainText('Beta');

  // Reorder from the keyboard: Alt+Up moves Alpha above Gamma.
  await rows(page).filter({ hasText: 'Alpha renamed' }).getByRole('button').first().focus();
  await page.keyboard.press('Alt+ArrowUp');
  await expect(rows(page)).toHaveText([/Beta/, /Alpha renamed/, /Gamma/]);
  await page.reload();
  await expect(rows(page)).toHaveText([/Beta/, /Alpha renamed/, /Gamma/]);

  // Move Gamma to the other project.
  await rows(page).filter({ hasText: 'Gamma' }).getByRole('button', { name: 'More actions' }).click();
  await page.getByRole('menuitem', { name: 'Move to another project' }).click();
  await page.getByRole('alertdialog').getByRole('button', { name: 'Move' }).click();
  await expect(rows(page).filter({ hasText: 'Gamma' })).toHaveCount(0);
  expect((await sessions()).find((x) => x.id === c.id)?.workspace.toLowerCase()).toBe(otherReal);

  // Delete asks first.
  await rows(page).filter({ hasText: 'Alpha renamed' }).getByRole('button', { name: 'More actions' }).click();
  await page.getByRole('menuitem', { name: 'Delete' }).click();
  await page.getByRole('alertdialog').getByRole('button', { name: 'Delete' }).click();
  await expect(rows(page).filter({ hasText: 'Alpha renamed' })).toHaveCount(0);
  expect((await sessions()).some((x) => x.id === a.id)).toBe(false);
});

test('the project panel saves notes to LUMEN.md, and the sidebar hides with Ctrl+B', async ({ page }) => {
  await page.goto(daemon.url());
  await page.getByRole('button', { name: /^Project: / }).click();
  await page.getByRole('menuitem', { name: 'Fixture' }).click();
  await rows(page).first().getByRole('button').first().click();
  await page.getByRole('button', { name: 'Project details' }).click();
  const panel = page.getByRole('complementary', { name: 'Project details' });
  await panel.getByRole('textbox', { name: 'Notes for every agent' }).fill('Use pnpm. Never push to main.');
  await panel.getByRole('button', { name: 'Save notes' }).click();
  await expect(panel.getByText('Saved', { exact: true })).toBeVisible();
  expect(await readFile(path.join(daemon.workspace, 'LUMEN.md'), 'utf8')).toContain('Use pnpm. Never push to main.');
  await expect(panel.getByText('Nothing recorded yet.')).toBeVisible();
  expect(await seriousViolations(page)).toEqual([]);
  await page.keyboard.press('Control+b');
  await expect(nav(page)).toHaveCount(0);
  await page.getByRole('button', { name: 'Show sidebar' }).click();
  await expect(nav(page)).toBeVisible();
});

test('500 conversations: search finds one at once, and the rest come a page at a time', async ({ page }) => {
  test.setTimeout(120_000);
  for (let i = 0; i < 500; i++) {
    const chat = (await daemon.client.call('create_session', { workspace: daemon.workspace })) as { id: string };
    await daemon.client.call('update_session', { id: chat.id, title: `Task ${String(i).padStart(3, '0')}`, pinned: false });
  }
  await page.goto(daemon.url());
  await expect(rows(page).first()).toBeVisible();
  const first = await rows(page).count();
  expect(first).toBeLessThanOrEqual(100);
  await nav(page).getByRole('button', { name: 'Show more' }).click();
  await expect.poll(() => rows(page).count()).toBeGreaterThan(first);
  const start = Date.now();
  await page.keyboard.press('Control+k');
  await page.keyboard.type('Task 042');
  await expect(rows(page)).toHaveCount(1);
  await expect(rows(page).first()).toContainText('Task 042');
  expect(Date.now() - start).toBeLessThan(2_000);
  await page.keyboard.press('Escape');
});
