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
