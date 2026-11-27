import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { startDaemon } from './daemon-fixture';

test.skip(process.platform !== 'win32', 'CLI fixtures are Windows-only');

let daemon: Awaited<ReturnType<typeof startDaemon>>;
test.beforeEach(async ({ baseURL }) => {
  daemon = await startDaemon(new URL(baseURL!).origin, { welcome: true });
});
test.afterEach(async () => daemon?.close());

async function accessible(page: Page) {
  const { violations } = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa', 'wcag22aa']).analyze();
  expect(violations.map((v) => ({ id: v.id, targets: v.nodes.map((n) => n.target) }))).toEqual([]);
}

test('configures appearance, verifies CLI sign-in, opens a folder and finishes the tour', async ({ page }) => {
  const codex = await daemon.client.call('add_account', { provider: 'gpt', name: 'Codex fixture' });
  await writeFile(path.join(daemon.data, 'accounts', codex.id, 'signed-out-fixture'), '');
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto(daemon.url({ locale: '' }));
  const welcome = page.getByRole('heading', { name: 'Welcome to Lumen' });
  await expect(welcome).toBeFocused();
  await expect(page.getByRole('main').locator('circle')).toHaveCSS('animation-name', 'none');
  await accessible(page);
  await page.keyboard.press('Tab');
  await page.keyboard.press('Tab');
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'Make it yours' })).toBeFocused();
  await page.getByRole('button', { name: /Language/ }).click();
  await page.getByRole('option', { name: 'Italiano' }).click();
  await expect(page.locator('html')).toHaveAttribute('lang', 'it');
  await page.getByRole('radio', { name: 'Satin' }).click({ force: true });
  await page.getByRole('radio', { name: 'Scuro' }).click({ force: true });
  await expect(page.locator('html')).toHaveAttribute('data-style', 'satin');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await accessible(page);
  await page.getByRole('button', { name: /Lingua/ }).click();
  await page.getByRole('option', { name: 'English' }).click();
  await page.getByRole('button', { name: 'Continue' }).click();

  const claude = page.getByRole('listitem').filter({ hasText: 'Claude Code' });
  await expect(claude).toContainText('Installed · 2.1.0 · No account added');
  await expect(page.getByRole('listitem').filter({ hasText: 'Droid' })).toContainText('Not installed');
  await expect(page.getByRole('listitem').filter({ hasText: 'Codex fixture' })).toContainText('Not connected');
  await claude.getByRole('button', { name: 'Sign in', exact: true }).click();
  await expect(claude).toContainText('Ready', { timeout: 20_000 });
  await accessible(page);
  await page.getByRole('button', { name: 'Continue' }).click();
  page.once('dialog', (dialog) => dialog.accept(daemon.workspace));
  await page.getByRole('button', { name: 'Choose a folder' }).click();
  await expect(page.getByText(daemon.workspace, { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Continue' }).click();
  await expect(page.getByText('Verified agents: Claude Code')).toBeVisible();
  await expect(page.getByText('Selected project: workspace')).toBeVisible();
  await expect(page.getByText('Auto is selected by default')).toBeVisible();
  await accessible(page);

  await page.getByRole('button', { name: 'Start quick tour' }).click();
  await expect(page.getByRole('heading', { name: 'Composer' })).toBeFocused();
  await page.getByRole('button', { name: 'New task' }).click();
  await expect(page.locator('[data-tour-highlight][data-tour-target="composer"]')).toBeVisible();
  for (let index = 0; index < 7; index++) await page.getByRole('button', { name: 'Continue' }).click();
  await expect(page.getByRole('region', { name: 'Settings' }).getByRole('heading', { name: 'Maintenance' })).toHaveAttribute('data-tour-highlight', '');
  await accessible(page);
  await page.getByRole('button', { name: 'Finish tour' }).click();
  await expect(page.getByRole('button', { name: 'Finish tour' })).toBeHidden();
  expect(await daemon.client.call('onboarding_state')).toMatchObject({ existing: false, setup: 'completed', tour: 'completed' });
});

test('skip survives a daemon restart and Settings reopen setup and tour', async ({ page }) => {
  await page.goto(daemon.url());
  await page.getByRole('button', { name: 'Skip setup' }).click();
  await expect(page.getByRole('heading', { name: 'Welcome to Lumen' })).toBeHidden();
  await daemon.stop();
  await daemon.restart();
  expect(await daemon.client.call('onboarding_state')).toMatchObject({ existing: false, setup: 'skipped' });
  await page.reload();
  await page.getByRole('button', { name: 'Settings' }).click();
  await page.getByRole('radio', { name: 'General' }).click({ force: true });
  await page.getByRole('button', { name: 'Start quick tour' }).click();
  await page.getByRole('button', { name: 'Skip tour' }).click();
  await expect(page.getByRole('button', { name: 'Skip tour' })).toBeHidden();
  expect(await daemon.client.call('onboarding_state')).toMatchObject({ tour: 'skipped' });
  await page.getByRole('button', { name: 'Settings' }).click();
  await page.getByRole('radio', { name: 'General' }).click({ force: true });
  await page.getByRole('button', { name: 'Open guided setup' }).click();
  await expect(page.getByRole('heading', { name: 'Welcome to Lumen' })).toBeFocused();
});

test('an existing database shows a dismissible invitation alongside its project', async ({ page }) => {
  await daemon.client.call('add_workspace', { name: 'Existing', folder: daemon.workspace });
  await daemon.stop();
  const database = new DatabaseSync(path.join(daemon.data, 'lumen.db'));
  database.exec('DROP TABLE onboarding');
  database.close();
  await daemon.restart();
  await page.goto(daemon.url());
  await expect(page.getByText('New in Lumen: guided setup')).toBeVisible();
  await expect(page.getByRole('button', { name: 'New task' })).toBeEnabled();
  await page.getByRole('button', { name: 'Dismiss welcome' }).click();
  await expect(page.getByText('New in Lumen: guided setup')).toBeHidden();
  expect(await daemon.client.call('onboarding_state')).toMatchObject({ existing: true, setup: 'skipped' });
});
