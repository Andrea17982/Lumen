// M10 against the real daemon: a task that names a branch runs in its own worktree; the bar shows
// it when enabled, Publish asks for a GitHub remote, Apply via IPC copies files into the folder and
// Discard returns to the project and keeps the task files.
import { expect, test, type Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { realpath, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { startDaemon } from './daemon-fixture';

test.skip(process.platform !== 'win32', 'CLI fixtures are Windows-only');
test.describe.configure({ mode: 'serial' });

let daemon: Awaited<ReturnType<typeof startDaemon>>;
const git = (...args: string[]) => execFileSync('git', ['-c', 'user.name=Lumen Test', '-c', 'user.email=test@example.invalid', ...args], { cwd: daemon.workspace, windowsHide: true, encoding: 'utf8' });
test.beforeAll(async ({ baseURL }) => {
  daemon = await startDaemon(new URL(baseURL!).origin);
  const account = await daemon.client.call('add_account', { name: 'Claude fixture', provider: 'claude' }) as { id: string };
  await daemon.client.call('set_claude_models', { id: account.id, value: { fable: false, mythos: false } });
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
  await expect(page.locator('main header')).toContainText('Done', { timeout: 60_000 });
  await expect(bar(page)).toHaveCount(0);
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('radio', { name: 'General' }).click({ force: true });
  await page.getByRole('switch', { name: 'Show Publish PR and Undo changes' }).click({ force: true });
  await page.getByRole('button', { name: 'Back to the conversation' }).click();
  await expect(bar(page)).toContainText(branch, { timeout: 60_000 });
  await expect(page.locator('main header')).not.toContainText(/Running|Queued/, { timeout: 60_000 });
  return chat;
}

test('a task on a named branch shows its bar, and Publish explains what it needs', async ({ page }) => {
  test.setTimeout(150_000);
  const chat = await start(page, 'feat-apply');
  await expect(bar(page)).toContainText(/\d+ commits?/);
  await daemon.stop();
  await daemon.restart();
  await page.reload();
  await expect(page.getByRole('navigation').getByRole('button', { name: /^Work on branch feat-apply/ })).toHaveAttribute('aria-current', 'page');
  await expect(bar(page)).toContainText('feat-apply');
  await bar(page).getByRole('button', { name: 'Publish PR' }).click();
  await expect(bar(page).getByRole('alert')).toContainText('Configure a GitHub origin');
  await expect(bar(page).getByRole('button', { name: 'Apply to the folder' })).toHaveCount(0);

  await daemon.client.call('apply_task', { id: chat.id });
  await page.reload();
  await expect(bar(page)).toHaveCount(0, { timeout: 30_000 });
  expect(existsSync(path.join(daemon.workspace, 'fixture-claude.txt'))).toBe(true);
});

test('Discard retains the task files and returns the same chat to the project', async ({ page }) => {
  test.setTimeout(150_000);
  git('stash', '-u', '-q');
  const chat = await start(page, 'feat-discard');
  const task = (await daemon.client.call('task_worktree_status', { id: chat.id, preview: false })) as { path: string } | null;
  if (!task) throw new Error('Expected an active task');
  await bar(page).getByRole('button', { name: 'Undo changes' }).click();
  const confirm = page.getByRole('alertdialog');
  await expect(confirm).toContainText(task.path);
  await confirm.getByRole('button', { name: 'Undo changes' }).click();
  await expect(bar(page)).toHaveCount(0, { timeout: 30_000 });
  expect(existsSync(path.join(daemon.workspace, 'fixture-claude.txt'))).toBe(false);
  expect(existsSync(path.join(task.path, 'fixture-claude.txt'))).toBe(true);
  const sessions = (await daemon.client.call('list_sessions', { search: '', offset: 0 })) as { id: string; workspace: string }[];
  const reopened = sessions.find((session) => session.id === chat.id);
  if (!reopened) throw new Error('Expected the same conversation after Discard');
  expect(await realpath(reopened.workspace)).toBe(await realpath(daemon.workspace));
  await page.reload();
  await page.getByRole('navigation').getByRole('button', { name: /^Work on branch feat-discard/ }).click();
  await expect(page.getByRole('log').getByText('Work on branch feat-discard: WRITE_FIXTURE', { exact: true })).toBeVisible();
});
