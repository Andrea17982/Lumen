import { expect, test } from '@playwright/test';

test('settings is modal for keyboard and accessibility and restores a usable opener', async ({
  page,
}) => {
  await page.goto('/');
  const settings = page.getByRole('button', { name: 'Settings', exact: true });
  await settings.click();
  const panel = page.getByRole('dialog', { name: 'Settings panel' });
  await expect(panel).toHaveAttribute('aria-modal', 'true');
  expect(
    await page.locator('.workspace').evaluate((element) => (element as HTMLElement).inert),
  ).toBe(true);
  expect(
    await page.locator('.sidebar-toggle').evaluate((element) => (element as HTMLElement).inert),
  ).toBe(true);
  await expect(page.getByRole('textbox', { name: 'Message', exact: true })).toHaveCount(0);
  const close = panel.getByRole('button', { name: 'Close settings' });
  await expect(close).toBeFocused();
  await page.keyboard.press('Control+p');
  await page.keyboard.press('Control+k');
  await expect(page.getByRole('dialog', { name: 'Command palette' })).toHaveCount(0);
  await expect(page.locator('.sidebar-toggle')).toHaveAttribute('aria-expanded', 'false');
  await close.press('Shift+Tab');
  expect(await panel.evaluate((element) => element.contains(document.activeElement))).toBe(true);
  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);
  await expect(settings).toBeFocused();
  await expect(page.locator('.sidebar-toggle')).toHaveAttribute('aria-expanded', 'false');
  await page.locator('.sidebar-toggle').click();
  await expect(page.getByRole('textbox', { name: 'Search conversations' })).toBeVisible();
});

test('narrow navigation cannot cover modal settings and reduced-motion close restores access', async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/');
  await page.locator('.sidebar-toggle').click();
  await page.keyboard.press('Control+k');
  const palette = page.getByRole('dialog', { name: 'Command palette' });
  await palette.getByRole('combobox').fill('settings');
  await palette.getByRole('option').first().click();
  const panel = page.getByRole('dialog', { name: 'Settings panel' });
  await expect(panel).toBeVisible();
  await expect(page.locator('.sidebar-scrim')).toHaveCount(0);
  await expect(page.locator('.sidebar-toggle')).toHaveAttribute('aria-expanded', 'false');
  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);
  await expect(page.getByRole('textbox', { name: 'Message', exact: true })).toBeVisible();
});
