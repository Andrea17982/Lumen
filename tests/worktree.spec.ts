import { expect, test, type Page } from '@playwright/test';

async function seed(page: Page, blocked: string | null) {
  await page.addInitScript(
    (blocked) =>
      localStorage.setItem(
        'lumen-preview-v1',
        JSON.stringify({
          accounts: [],
          sessions: [
            {
              id: 'task',
              title: 'Add the login page',
              status: 'completed',
              workspace: 'C:/Data/worktrees/task/task',
              updatedAt: 1,
            },
          ],
          messages: {
            task: [{ id: 'a', role: 'assistant', text: 'Login page added.', activity: [] }],
          },
          tasks: {
            task: {
              branch: 'lumen/add-the-login-page-a1b2c3',
              origin: 'C:/Work/app',
              path: 'C:/Data/worktrees/task/task',
              ahead: 2,
              dirty: false,
              remote: 'https://github.com/example/app.git',
              base: 'main',
              blocked,
            },
          },
        }),
      ),
    blocked,
  );
  await page.goto('/');
  await page.getByRole('button', { name: /^(?!Actions for).*Add the login page/ }).click();
}

test('a task worktree shows its branch and publishes after a preview', async ({ page }) => {
  await seed(page, null);
  await expect(page.locator('.worktree-chip')).toContainText('lumen/add-the-login-page-a1b2c3');
  const bar = page.getByTestId('task-worktree');
  await expect(bar).toContainText('2 commits');
  await bar.getByRole('button', { name: 'Publish' }).click();
  const dialog = page.getByRole('dialog', { name: 'Publish' });
  await expect(dialog).toContainText('https://github.com/example/app.git');
  await expect(dialog.getByLabel('Base branch')).toHaveValue('main');
  await expect(dialog.getByLabel('Title')).toHaveValue('Add the login page');
  await dialog.getByRole('button', { name: 'Publish' }).click();
  await expect(page.getByRole('status')).toContainText('example/project/pull/1');
  await expect(bar).toHaveCount(0);
  await expect(page.locator('.worktree-chip')).toHaveCount(0);
});

test('Publish stays disabled with the reason when GitHub CLI is unavailable', async ({ page }) => {
  await seed(page, 'GitHub CLI (gh) is not installed');
  await page.getByTestId('task-worktree').getByRole('button', { name: 'Publish' }).click();
  const dialog = page.getByRole('dialog', { name: 'Publish' });
  await expect(dialog).toContainText('GitHub CLI (gh) is not installed');
  await expect(dialog.getByRole('button', { name: 'Publish' })).toBeDisabled();
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await page.getByTestId('task-worktree').getByRole('button', { name: 'Discard' }).click();
  await page
    .getByRole('dialog', { name: 'Discard' })
    .getByRole('button', { name: 'Discard' })
    .click();
  await expect(page.getByTestId('task-worktree')).toHaveCount(0);
});
