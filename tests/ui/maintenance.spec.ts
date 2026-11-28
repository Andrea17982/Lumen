// 0.15 against the real daemon: an account signs in without a terminal and shows its CLI, and
// Repair Lumen checks the service and the database, backs it up and reports each agent.
import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { existsSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { startDaemon } from './daemon-fixture';

test.skip(process.platform !== 'win32', 'CLI fixtures are Windows-only');
test.describe.configure({ mode: 'serial' });

let daemon: Awaited<ReturnType<typeof startDaemon>>;
test.beforeAll(async ({ baseURL }) => {
  daemon = await startDaemon(new URL(baseURL!).origin);
  await daemon.client.call('add_account', { name: 'Claude fixture', provider: 'claude' });
});
test.afterAll(async () => daemon?.close());

const settings = (page: Page) => page.getByRole('region', { name: 'Settings' });
async function openSettings(page: Page, section: string) {
  await page.goto(daemon.url());
  await page.getByRole('button', { name: 'Settings' }).click();
  await settings(page).getByRole('radio', { name: section }).click({ force: true });
  await expect(settings(page).getByRole('heading', { name: section })).toBeVisible();
}
async function accessible(page: Page) {
  const { violations } = await new AxeBuilder({ page }).include('main').withTags(['wcag2a', 'wcag2aa', 'wcag21aa', 'wcag22aa']).analyze();
  expect(violations.map((v) => v.id)).toEqual([]);
}

test('an account signs in without a terminal and shows its CLI', async ({ page }) => {
  await openSettings(page, 'Accounts');
  const card = settings(page).getByRole('listitem', { name: /Claude fixture/ });
  await page.getByRole('alertdialog', { name: 'Models included in Claude fixture' }).getByRole('button', { name: 'Save' }).click();
  await expect(card).toContainText('Claude Code');
  await card.getByRole('button', { name: 'More actions' }).click();
  await page.getByRole('menuitem', { name: 'Sign in again' }).click();
  // The offline CLI signs in at once; Lumen confirms it with the CLI's own status and closes the flow.
  await expect(card.getByRole('button', { name: 'Check' })).toBeVisible({ timeout: 20_000 });
  await expect(card).toContainText('Connected');
  await accessible(page);
  await page.screenshot({ path: test.info().outputPath('accounts.png'), fullPage: true });
});

test('Repair checks the service and the database, backs it up and reports each agent', async ({ page }) => {
  await openSettings(page, 'Maintenance');
  await settings(page).getByRole('button', { name: 'Repair Lumen' }).click();
  const report = settings(page).getByRole('list', { name: 'Repair Lumen' });
  await expect(report).toContainText('Service answering', { timeout: 30_000 });
  await expect(report).toContainText('Database checked, backup saved');
  await expect(report).toContainText(/Claude fixture \(Claude Code\).*signed in/);
  await expect(report).toContainText('No leftover task folders');
  const backups = path.join(daemon.data, 'backups');
  expect(existsSync(backups) && readdirSync(backups).some((f) => /^lumen-\d+\.db$/.test(f))).toBe(true);
  await accessible(page);
  await page.screenshot({ path: test.info().outputPath('maintenance.png'), fullPage: true });
});

test('the update banner keeps Later and Update together on the right, Update last', async ({ page }) => {
  await page.goto(daemon.url());
  await expect(page.getByRole('button', { name: 'New task' })).toBeVisible();
  daemon.emit('lumen:update', { version: '9.9.9', notes: null, installable: true, page: 'https://github.com/Andrea17982/Lumen/releases' });
  const banner = page.getByRole('status').filter({ hasText: 'Lumen 9.9.9 is available.' });
  const text = (await banner.getByText('Lumen 9.9.9 is available.').boundingBox())!;
  const later = (await banner.getByRole('button', { name: 'Later' }).boundingBox())!;
  const update = (await banner.getByRole('button', { name: 'Update' }).boundingBox())!;
  const box = (await banner.boundingBox())!;
  expect(later.x).toBeGreaterThan(text.x + text.width);
  expect(update.x).toBeGreaterThan(later.x + later.width);
  // Adjacent, and against the right edge: not spread across the bar.
  expect(update.x - (later.x + later.width)).toBeLessThan(24);
  expect(box.x + box.width - (update.x + update.width)).toBeLessThan(40);
  await banner.getByRole('button', { name: 'Later' }).click();
  await expect(banner).toBeHidden();
});
