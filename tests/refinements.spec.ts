import { expect, test } from '@playwright/test';

test('fresh provider bar contains only Auto and no redundant project filter', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('group', { name: 'AI provider' }).getByRole('button')).toHaveText([
    'Auto',
  ]);
  await expect(page.getByRole('button', { name: 'Auto', exact: true })).toHaveAttribute(
    'aria-pressed',
    'true',
  );
  await expect(page.getByRole('button', { name: 'Advanced provider options' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'All conversations', exact: true })).toHaveCount(0);
  await expect(page.getByText('Enter to send')).toHaveCount(0);
});

test('glass selectors work by keyboard and About fits half-screen settings', async ({ page }) => {
  await page.setViewportSize({ width: 760, height: 820 });
  await page.addInitScript(() => localStorage.setItem('lumen-theme', 'dark'));
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const language = page.locator('.language-control [role=combobox]');
  await language.focus();
  await language.press('Enter');
  await expect(page.getByRole('option', { name: 'English', exact: true })).toBeVisible();
  await page
    .locator('.select-menu')
    .evaluate((el) => el.getAnimations().forEach((animation) => animation.finish()));
  await page.screenshot({ path: '.codex/followup-dark-select.png' });
  await language.press('End');
  await language.press('Enter');
  await expect(language).toHaveText('Italiano');
  await language.press('Enter');
  await language.press('Home');
  await language.press('Enter');
  await expect(language).toHaveText('English');
  await page.getByRole('button', { name: 'Advanced', exact: true }).click();
  const about = page.getByRole('tab', { name: 'About', exact: true });
  // Tabs stay on one scrollable row; the last one is reachable by scrolling.
  expect(
    await page
      .locator('.settings-tabs [role=tab]')
      .evaluateAll((tabs) => new Set(tabs.map((t) => t.getBoundingClientRect().top)).size),
  ).toBe(1);
  await about.scrollIntoViewIfNeeded();
  await expect(about).toBeInViewport();
  await about.click();
  await expect(page.getByRole('heading', { name: 'About Lumen' })).toBeVisible();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.screenshot({ path: '.codex/followup-half-screen.png' });
});

test('project rename/delete preserves conversations moved out first', async ({ page }) => {
  await page.addInitScript(() =>
    localStorage.setItem(
      'lumen-preview-v1',
      JSON.stringify({
        accounts: [],
        workspaces: [
          { id: 'a', name: 'First', folder: 'C:/Project' },
          { id: 'b', name: 'Second', folder: 'C:/Project' },
        ],
        sessions: [
          {
            id: 's1',
            title: 'Delete with project',
            status: 'idle',
            pinned: false,
            workspace: 'C:/Project',
            workspaceId: 'a',
            updatedAt: 1,
          },
          {
            id: 's2',
            title: 'Keep moved chat',
            status: 'idle',
            pinned: false,
            workspace: 'C:/Project',
            workspaceId: 'a',
            updatedAt: 2,
          },
        ],
      }),
    ),
  );
  await page.goto('/');
  await page
    .locator('.session-row')
    .filter({ hasText: 'Keep moved chat' })
    .click({ button: 'right' });
  await page.getByRole('combobox', { name: 'Move conversation to project' }).click();
  await page.getByRole('option', { name: 'Second', exact: true }).click();
  const project = page.locator('.project-list').getByRole('button', { name: 'First', exact: true });
  await project.click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Rename project' }).click();
  await page.getByRole('textbox', { name: 'Project name', exact: true }).fill('Renamed');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await page
    .locator('.project-list')
    .getByRole('button', { name: 'Renamed', exact: true })
    .click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Delete project' }).click();
  await page
    .getByRole('dialog', { name: 'Delete project' })
    .getByRole('button', { name: 'Delete project', exact: true })
    .click();
  await expect(page.locator('.session-row').filter({ hasText: 'Delete with project' })).toHaveCount(
    0,
  );
  await expect(page.locator('.session-row').filter({ hasText: 'Keep moved chat' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Unsorted', exact: true })).toHaveCount(0);
  await expect(page.locator('.session-menu-button')).toHaveCount(0);
});

test('connected providers all fit without horizontal scrolling in either theme', async ({
  page,
}) => {
  await page.setViewportSize({ width: 760, height: 820 });
  await page.addInitScript(() =>
    localStorage.setItem(
      'lumen-preview-v1',
      JSON.stringify({
        accounts: ['claude', 'gpt', 'antigravity', 'ollama'].map((provider) => ({
          id: provider,
          name: provider,
          provider,
        })),
        sessions: [],
      }),
    ),
  );
  await page.goto('/');
  const picker = page.getByRole('group', { name: 'AI provider' });
  for (const name of ['Auto', 'Claude', 'GPT', 'Antigravity', 'Ollama'])
    await expect(picker.getByRole('button', { name, exact: true })).toBeInViewport();
  expect(await picker.evaluate((el) => el.scrollWidth <= el.clientWidth)).toBe(true);
  await page.screenshot({ path: '.codex/followup-light.png' });
});
