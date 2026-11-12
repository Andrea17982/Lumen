// Windows WebView2 with disposable data; no accounts or inference.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { expect } from '@playwright/test';
import { nativeHarness } from './native-harness.mjs';

const h = await nativeHarness('sidebar');
const checks = [];
const output = path.join(h.root, '.impeccable/sidebar');
try {
  await mkdir(output, { recursive: true });
  const { page } = h;
  const logo = page.locator('.sidebar-toggle');
  const rail = page.locator('#conversation-sidebar');
  const input = page.getByRole('textbox', { name: 'Message', exact: true });
  await expect(logo).toHaveAttribute('aria-expanded', 'true');
  await input.fill('Preserve this draft across navigation.');
  await page.screenshot({ path: path.join(output, 'light-open.png') });
  await logo.click();
  await expect(rail).toHaveAttribute('inert', '');
  await expect(logo).toHaveAccessibleName('Open sidebar');
  await expect(input).toHaveValue('Preserve this draft across navigation.');
  await expect(rail).toBeHidden();
  await page.screenshot({ path: path.join(output, 'light-closed.png') });
  checks.push(
    'Desktop logo switches compositions while preserving the draft and hiding rail controls',
  );
  await logo.press('Enter');
  const search = page.getByRole('textbox', { name: 'Search conversations' });
  await search.focus();
  await page.keyboard.press('Escape');
  await expect(logo).toBeFocused();
  await page.keyboard.press('/');
  await expect(search).toBeFocused();
  checks.push('Enter, Escape and search shortcut preserve focus across disclosure');

  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'Dark', exact: true }).click();
  await page.getByRole('button', { name: 'Close settings', exact: true }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.screenshot({ path: path.join(output, 'dark-open.png') });
  await logo.click();
  await expect(rail).toBeHidden();
  await page.screenshot({ path: path.join(output, 'dark-closed.png') });
  checks.push('Dark theme preserves glass, composition and labelled logo state');

  await page.emulateMedia({ reducedMotion: 'reduce' });
  await logo.click();
  assert.deepEqual(
    await page.evaluate(() => ({
      transition: getComputedStyle(document.querySelector('.sidebar')).transitionDuration,
      animation: getComputedStyle(document.querySelector('.logo-glint')).animationName,
      opacity: getComputedStyle(document.querySelector('.logo-glint')).opacity,
    })),
    { transition: '0s', animation: 'none', opacity: '0' },
  );
  checks.push('Reduced motion removes both spatial transition and glint');

  await logo.click();
  await page.setViewportSize({ width: 390, height: 844 });
  await logo.click();
  await expect(search).toBeVisible();
  await page.screenshot({ path: path.join(output, 'narrow-open.png') });
  await page
    .getByRole('button', { name: 'Close navigation overlay' })
    .click({ position: { x: 350, y: 300 } });
  await expect(logo).toBeFocused();
  await expect(rail).toHaveAttribute('inert', '');
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
  await page.screenshot({ path: path.join(output, 'narrow-closed.png') });
  checks.push(
    'Narrow WebView viewport uses overlay dismissal and restores logo focus without horizontal overflow',
  );
  await writeFile(
    path.join(output, 'results.json'),
    JSON.stringify(
      {
        passed: true,
        checks,
        scope:
          'Windows debug WebView2, isolated data, real desktop viewport plus emulated narrow viewport; no screen-reader or inference certification.',
      },
      null,
      2,
    ),
  );
  process.stdout.write(JSON.stringify({ passed: true, checks }, null, 2) + '\n');
} finally {
  await h.close();
}
