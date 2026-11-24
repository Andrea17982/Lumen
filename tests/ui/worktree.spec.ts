// M10 against the real daemon: a task that names a branch runs in its own worktree; the bar shows
// it, Publish says why it cannot run without a remote, Apply brings the work into the folder and
// Discard deletes the branch.
import { expect, test, type Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { writeFile } from 'node:fs/promises';
import path from 'node:path';
import { startDaemon } from './daemon-fixture';

test.skip(process.platform !== 'win32', 'CLI fixtures are Windows-only');
test.describe.configure({ mode: 'serial' });

let daemon: Awaited<ReturnType<typeof startDaemon>>;
const git = (...args: string[]) => execFileSync('git', ['-c', 'user.name=Lumen Test', '-c', 'user.email=test@example.invalid', ...args], { cwd: daemon.workspace, windowsHide: true, encoding: 'utf8' });
test.beforeAll(async ({ baseURL }) => {
  daemon = await startDaemon(new URL(baseURL!).origin);
  await daemon.client.call('add_account', { name: 'Claude fixture', provider: 'claude' });
  git('init', '-q', '-b', 'main');
  await writeFile(path.join(daemon.workspace, 'README.md'), 'fixture\n');
  git('add', '.');
  git('commit', '-q', '-m', 'start');
});
test.afterAll(async () => daemon?.close());

const bar = (page: Page) => page.getByRole('region', { name: 'Task branch' });

async function start(page: Page, branch: string) {
  git('branch', branch);
  const chat = (await daemon.client.call('create_session', { workspace: daemon.workspace })) as { id: string };
  await daemon.client.call('set_session_access_mode', { id: chat.id, mode: 'file-edits' });
  await page.goto(daemon.url());
  await page.getByRole('navigation').getByRole('listitem').first().getByRole('button').first().click();
  const box = page.getByRole('textbox', { name: 'Describe what you want to do' });
  await box.fill(`Work on branch ${branch}: WRITE_FIXTURE`);
  await box.press('Enter');
  await expect(bar(page)).toContainText(branch, { timeout: 60_000 });
  await expect(page.locator('main header')).not.toContainText(/Running|Queued/, { timeout: 60_000 });
}

test('a task on a named branch shows its bar, and Publish explains what it needs', async ({ page }) => {
  test.setTimeout(150_000);
  await start(page, 'feat-apply');
  await expect(bar(page)).toContainText(/\d+ commits?/);
  await bar(page).getByRole('button', { name: 'Publish…' }).click();
  const dialog = page.getByRole('alertdialog');
  await expect(dialog).toContainText('Publish as a pull request');
  // No origin remote here: the daemon says so before anything is pushed.
  await expect(dialog.getByRole('alert')).toBeVisible();
  await dialog.getByRole('button', { name: 'Cancel' }).click();

  await bar(page).getByRole('button', { name: 'Apply to the folder' }).click();
  await page.getByRole('alertdialog').getByRole('button', { name: 'Apply to the folder' }).click();
  await expect(bar(page)).toHaveCount(0, { timeout: 30_000 });
  expect(existsSync(path.join(daemon.workspace, 'fixture-claude.txt'))).toBe(true);
});

test('Discard deletes the task branch and leaves the folder as it was', async ({ page }) => {
  test.setTimeout(150_000);
  git('stash', '-u', '-q');
  await start(page, 'feat-discard');
  await bar(page).getByRole('button', { name: 'Discard' }).click();
  await page.getByRole('alertdialog').getByRole('button', { name: 'Discard' }).click();
  await expect(bar(page)).toHaveCount(0, { timeout: 30_000 });
  expect(existsSync(path.join(daemon.workspace, 'fixture-claude.txt'))).toBe(false);
});
