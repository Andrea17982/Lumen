import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';

const styles = ['ink', 'satin'] as const;
const themes = ['light', 'dark'] as const;
const views = ['app', 'board', 'components'] as const;

async function open(page: Page, query: Record<string, string>) {
  await page.goto('/gallery.html?' + new URLSearchParams({ toolbar: '0', locale: 'en', ...query }));
  await expect(page.locator('html')).toHaveAttribute('data-style', query.style ?? 'ink');
}

async function seriousViolations(page: Page) {
  const { violations } = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa']).analyze();
  return violations.filter((v) => v.impact === 'serious' || v.impact === 'critical').map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`);
}

for (const style of styles)
  for (const theme of themes)
    for (const view of views)
      test(`axe: ${view} in ${style}/${theme} has no serious violations`, async ({ page }) => {
        await open(page, { style, theme, view });
        expect(await seriousViolations(page)).toEqual([]);
      });

test('axe: Italian and Windows high contrast', async ({ page }) => {
  await page.emulateMedia({ forcedColors: 'active' });
  for (const view of views) {
    await open(page, { view, locale: 'it' });
    expect(await seriousViolations(page), view).toEqual([]);
  }
});

test('at 150% Windows text size the app has no horizontal overflow', async ({ page }) => {
  // WebView2 applies the text scale as page zoom: a 1240×820 window shows 827×547 CSS pixels.
  await page.setViewportSize({ width: 827, height: 547 });
  for (const locale of ['en', 'it']) {
    await open(page, { view: 'app', locale });
    const overflow = await page.evaluate(() => [...document.querySelectorAll('aside *, main header, footer')].filter((e) => e.scrollWidth > e.clientWidth + 1 && getComputedStyle(e).overflow === 'visible').map((e) => e.className));
    expect(overflow, locale).toEqual([]);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  }
});

test('appearance controls change the style and theme, and the choice persists', async ({ page }) => {
  await open(page, { view: 'components' });
  await page.getByRole('radio', { name: 'Satin' }).or(page.getByRole('button', { name: 'Satin' })).click();
  await expect(page.locator('html')).toHaveAttribute('data-style', 'satin');
  await page.getByRole('button', { name: /Theme/ }).click();
  await page.getByRole('option', { name: 'Dark' }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.goto('/gallery.html?toolbar=0&view=components&locale=en');
  await expect(page.locator('html')).toHaveAttribute('data-style', 'satin');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});

test('menu, dialog, switch and search work from the keyboard', async ({ page }) => {
  await open(page, { view: 'components' });
  const more = page.getByRole('button', { name: 'More actions' });
  await more.focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('menuitem', { name: 'Rename' })).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('Enter');
  await expect(page.getByText('pin', { exact: true })).toBeVisible();

  await page.getByRole('button', { name: 'Delete…' }).click();
  const dialog = page.getByRole('alertdialog', { name: 'Delete this conversation?' });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole('button', { name: 'Cancel' })).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  await expect(page.getByRole('button', { name: 'Delete…' })).toBeFocused();

  const verify = page.getByRole('switch', { name: 'Check changes after every turn' });
  await expect(verify).toBeChecked();
  await verify.focus();
  await page.keyboard.press('Space');
  await expect(verify).not.toBeChecked();

  const search = page.getByRole('searchbox', { name: 'Search' });
  await search.fill('quota');
  await page.getByRole('button', { name: 'Clear search' }).click();
  await expect(search).toHaveValue('');
});

test('invalid fields announce their error', async ({ page }) => {
  await open(page, { view: 'components' });
  const invalid = page.getByRole('textbox', { name: 'Account name' }).nth(1);
  await expect(invalid).toHaveAttribute('aria-invalid', 'true');
  await expect(invalid).toHaveAccessibleDescription(/1–80 characters/);
});

test.describe('screenshots', () => {
  test.skip(process.platform !== 'win32', 'Baselines are recorded on Windows only');
  for (const style of styles)
    for (const theme of themes)
      for (const view of ['app', 'components'] as const)
        test(`${view} ${style} ${theme}`, async ({ page }) => {
          await open(page, { style, theme, view });
          await expect(page).toHaveScreenshot(`${view}-${style}-${theme}.png`, { fullPage: false });
        });
});
