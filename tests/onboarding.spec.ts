import { expect, test } from '@playwright/test';
import { mkdir } from 'node:fs/promises';

test('permission explanations open without granting access; keyboard chooses explicitly', async ({
  page,
}) => {
  await page.goto('/');
  const control = page.getByRole('button', { name: 'Permissions: Read only', exact: true });
  await expect(control).toContainText('Read only');
  for (const width of [720, 827, 1240]) {
    await page.setViewportSize({ width, height: 820 });
    const box = await control.boundingBox();
    const send = await page
      .getByRole('button', { name: 'Send message', exact: true })
      .boundingBox();
    expect(Math.abs(box!.y - send!.y)).toBeLessThan(10);
  }
  await control.click();
  const menu = page.getByRole('menu', { name: 'Choose agent permissions' });
  await expect(menu).toContainText('Choose what the agent is allowed to do');
  await expect(menu.getByRole('menuitemradio', { name: /^Read only/ })).toHaveAttribute(
    'aria-checked',
    'true',
  );
  await expect(control).toHaveAccessibleName('Permissions: Read only');
  await page.keyboard.press('Escape');
  await expect(control).toBeFocused();
  await control.click();
  await page.keyboard.press('End');
  await page.keyboard.press('Enter');
  await expect(
    page.getByRole('button', { name: 'Permissions: Autopilot', exact: true }),
  ).toBeFocused();
  await expect(menu).toHaveCount(0);
});

test('first-use tour completes all real targets and preserves draft and permissions', async ({
  page,
}) => {
  await page.goto('/');
  const input = page.getByRole('textbox', { name: 'Message', exact: true });
  await input.fill('Keep my draft');
  await page.getByRole('button', { name: 'Take a quick tour', exact: true }).click();
  const card = page.getByRole('dialog');
  const targets = [
    '.sidebar-toggle',
    '[data-tour="settings"]',
    '.mode-control',
    '[data-tour="permissions"]',
    '.composer',
  ];
  for (let index = 0; index < targets.length; index++) {
    await expect(card.locator('h2')).toBeFocused();
    await expect(card).toContainText(`${index + 1} / 5`);
    await expect(page.locator('.tour-highlight')).toBeVisible();
    const box = await card.boundingBox();
    expect(box!.x).toBeGreaterThanOrEqual(0);
    expect(box!.y).toBeGreaterThanOrEqual(0);
    await card
      .getByRole('button', { name: index === 4 ? 'Start exploring' : 'Next', exact: true })
      .click();
  }
  await expect(card).toHaveCount(0);
  await expect(input).toHaveValue('Keep my draft');
  await expect(
    page.getByRole('button', { name: 'Permissions: Read only', exact: true }),
  ).toBeVisible();
  expect(await page.evaluate(() => localStorage.getItem('lumen-tour-v1'))).toBe('completed');
  expect(await page.locator('.message').count()).toBe(0);
  await page.reload();
  await expect(page.getByRole('button', { name: 'Take a quick tour', exact: true })).toHaveCount(0);
});

test('skip persists; Settings can replay and Escape restores a usable composer', async ({
  page,
}) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Skip tour', exact: true }).click();
  await page.reload();
  await expect(page.getByRole('button', { name: 'Skip tour', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'Take a quick tour', exact: true }).click();
  await expect(page.locator('.tour-card h2')).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(page.locator('.tour-card')).toHaveCount(0);
  await expect(page.getByRole('textbox', { name: 'Message', exact: true })).toBeFocused();
  await expect(page.locator('.workspace')).not.toHaveAttribute('inert');
});

test('Italian tour and permissions fit a narrow dark window with reduced motion', async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 700 });
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.addInitScript(() => {
    localStorage.setItem('lumen-language', 'it');
    localStorage.setItem('lumen-theme', 'dark');
  });
  await page.goto('/');
  const permissions = page.getByRole('button', { name: 'Permessi: Sola lettura', exact: true });
  await expect(permissions).toContainText('Sola lettura');
  await page.getByRole('button', { name: 'Scopri Lumen in pochi passi', exact: true }).click();
  for (let i = 0; i < 4; i++)
    await page.getByRole('button', { name: 'Avanti', exact: true }).click();
  const card = page.locator('.tour-card');
  await expect(card).toContainText('Chiedi e segui il lavoro');
  const box = await card.boundingBox();
  expect(box!.x + box!.width).toBeLessThanOrEqual(390);
  expect(box!.y + box!.height).toBeLessThanOrEqual(700);
  expect(await card.evaluate((e) => getComputedStyle(e).animationName)).toBe('none');
  await mkdir('.impeccable/review', { recursive: true });
  await page.screenshot({ path: '.impeccable/review/tour-italian-dark.png' });
  await page.keyboard.press('Escape');
  await permissions.click();
  await expect(page.getByRole('menu')).toContainText('Scegli cosa può fare l’agente');
  await page.screenshot({ path: '.impeccable/review/permissions-italian-dark.png' });
  const send = await page
    .getByRole('button', { name: 'Invia messaggio', exact: true })
    .boundingBox();
  const access = await permissions.boundingBox();
  expect(Math.abs(send!.y - access!.y)).toBeLessThan(10);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test('returning accounts do not receive the first-use invitation', async ({ page }) => {
  await page.addInitScript(() =>
    localStorage.setItem(
      'lumen-preview-v1',
      JSON.stringify({
        accounts: [{ id: 'a', name: 'Existing', provider: 'claude' }],
        sessions: [],
      }),
    ),
  );
  await page.goto('/');
  await expect(page.getByRole('button', { name: 'Take a quick tour', exact: true })).toHaveCount(0);
  await expect
    .poll(() => page.evaluate(() => localStorage.getItem('lumen-tour-v1')))
    .toBe('returning-user');
});
