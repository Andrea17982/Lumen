import { expect, test } from '@playwright/test';

test('first run, provider controls and honest browser boundary', async ({ page }) => {
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto('/');
  await expect(
    page.getByRole('heading', { name: 'A clear space for a brighter idea.' }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Build something' }).click();
  await expect(page.getByRole('textbox', { name: 'Message', exact: true })).not.toBeEmpty();
  await page.getByRole('button', { name: 'Send message' }).click();
  await expect(page.getByRole('alert')).toContainText('browser preview');
  await page.getByRole('button', { name: 'Claude', exact: true }).click();
  await page.getByRole('button', { name: 'Advanced provider options' }).click();
  await expect(page.getByRole('textbox', { name: 'Model', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Auto', exact: true }).click();
  await expect(page.getByRole('textbox', { name: 'Model', exact: true })).not.toBeVisible();
  expect(errors).toEqual([]);
});

test('settings, theme and support information persist across reloads', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Connected accounts' })).toBeVisible();
  await page.getByRole('button', { name: 'Dark', exact: true }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await expect(page.getByRole('button', { name: 'Support Lumen on PayPal' })).toBeVisible();
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});

test('keyboard navigation and narrow layout stay usable', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Open sidebar' }).click();
  await expect(page.getByRole('textbox', { name: 'Search conversations' })).toBeVisible();
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'Explore an idea' }).click();
  await page.keyboard.press('Control+n');
  await expect(page.getByRole('textbox', { name: 'Message', exact: true })).toBeFocused();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test('settings contains keyboard focus and restores it; search shortcut works', async ({
  page,
}) => {
  await page.goto('/');
  const settings = page.getByRole('button', { name: 'Settings', exact: true });
  await settings.focus();
  await page.keyboard.press('Enter');
  const panel = page.getByRole('dialog', { name: 'Settings panel' });
  await expect(panel.getByRole('button', { name: 'Workspace', exact: true })).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  await expect(panel.getByRole('button', { name: 'Support Lumen on PayPal' })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(panel.getByRole('button', { name: 'Workspace', exact: true })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);
  await expect(settings).toBeFocused();
  await page.keyboard.press('/');
  await expect(page.getByRole('textbox', { name: 'Search conversations' })).toBeFocused();
});

test('filtering the sidebar preserves the selected running conversation', async ({ page }) => {
  await page.addInitScript(() =>
    localStorage.setItem(
      'lumen-preview-v1',
      JSON.stringify({
        accounts: [],
        sessions: [
          {
            id: 'running-fixture',
            title: 'Running fixture',
            status: 'running',
            pinned: false,
            workspace: 'Fixture workspace',
            updatedAt: 1,
          },
        ],
      }),
    ),
  );
  await page.goto('/');
  await page.getByRole('button', { name: /Running fixture/ }).click();
  await page.getByRole('textbox', { name: 'Search conversations' }).fill('no matches');
  await expect(page.getByText('No conversations found.')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Stop response' })).toBeVisible();
  await expect(page.locator('.conversation-label')).toHaveText('Running fixture');
});

test('account errors remain visible inside settings', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'Add account', exact: true }).click();
  await page.getByRole('textbox', { name: 'Account name' }).fill('Preview account');
  await page.getByRole('button', { name: 'Add account', exact: true }).last().click();
  await expect(page.getByRole('dialog').getByRole('alert')).toContainText('desktop app');
});
