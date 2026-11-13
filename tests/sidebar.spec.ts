import { expect, test } from '@playwright/test';

test('Italian logo labels identify the controlled navigation region', async ({ page }) => {
  await page.setViewportSize({ width: 700, height: 844 });
  await page.addInitScript(() => localStorage.setItem('lumen-language', 'it'));
  await page.goto('/');
  const logo = page.getByRole('button', { name: 'Apri barra laterale', exact: true });
  await logo.click();
  await expect(page.locator('.sidebar-toggle')).toHaveAccessibleName('Chiudi barra laterale');
  await expect(page.getByRole('complementary', { name: 'Conversazioni e progetti' })).toBeVisible();
  await expect(page.locator('.sidebar-toggle')).toHaveAttribute(
    'aria-controls',
    'conversation-sidebar',
  );
});

test('logo toggles the desktop rail without losing a draft or exposing hidden controls', async ({
  page,
}) => {
  await page.goto('/');
  const logo = page.locator('.sidebar-toggle');
  const rail = page.locator('#conversation-sidebar');
  const input = page.getByRole('textbox', { name: 'Message', exact: true });
  await input.fill('Keep this draft while navigating');
  await expect(logo).toHaveAttribute('aria-expanded', 'true');
  await logo.click();
  await expect(logo).toHaveAccessibleName('Open sidebar');
  await expect(rail).toHaveAttribute('inert', '');
  await expect(page.getByRole('textbox', { name: 'Search conversations' })).toHaveCount(0);
  await expect(input).toHaveValue('Keep this draft while navigating');
  await logo.press('Enter');
  await expect(logo).toHaveAttribute('aria-expanded', 'true');
  const search = page.getByRole('textbox', { name: 'Search conversations' });
  await search.focus();
  await page.keyboard.press('Escape');
  await expect(logo).toBeFocused();
  await expect(rail).toHaveAttribute('inert', '');
  await page.keyboard.press('/');
  await expect(search).toBeFocused();
  await expect(input).toHaveValue('Keep this draft while navigating');
});

test('logo and overlay close narrow navigation with focus restored', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/');
  const logo = page.locator('.sidebar-toggle');
  await expect(logo).toHaveAccessibleName('Open sidebar');
  await logo.click();
  await expect(page.getByRole('textbox', { name: 'Search conversations' })).toBeVisible();
  await page
    .getByRole('button', { name: 'Close navigation overlay' })
    .click({ position: { x: 350, y: 300 } });
  await expect(logo).toBeFocused();
  await expect(logo).toHaveAttribute('aria-expanded', 'false');
  await logo.press('Space');
  await expect(logo).toHaveAttribute('aria-expanded', 'true');
  await logo.press('Space');
  await expect(logo).toHaveAttribute('aria-expanded', 'false');
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test('reduced motion suppresses glint and transition in either theme', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/');
  const logo = page.locator('.sidebar-toggle');
  for (const theme of ['light', 'dark']) {
    await page.evaluate((theme) => (document.documentElement.dataset.theme = theme), theme);
    await logo.click();
    await expect(logo).toHaveAttribute('aria-expanded', 'false');
    const motion = await page.evaluate(() => ({
      transition: getComputedStyle(document.querySelector('.sidebar')!).transitionDuration,
      animation: getComputedStyle(document.querySelector('.logo-glint')!).animationName,
      opacity: getComputedStyle(document.querySelector('.logo-glint')!).opacity,
    }));
    expect(motion).toEqual({ transition: '0s', animation: 'none', opacity: '0' });
    await logo.click();
    await expect(logo).toHaveAttribute('aria-expanded', 'true');
  }
});
