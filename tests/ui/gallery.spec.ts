import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';

const styles = ['ink', 'satin'] as const;
const themes = ['light', 'dark'] as const;
const views = ['app', 'board', 'components', 'attention', 'welcome'] as const;

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
  test('welcome in Ink light', async ({ page }) => {
    await open(page, { view: 'welcome', style: 'ink', theme: 'light' });
    await expect(page).toHaveScreenshot('welcome-ink-light.png', { animations: 'disabled' });
  });
  test('welcome appearance in Satin dark', async ({ page }) => {
    await open(page, { view: 'welcome', step: 'appearance', style: 'satin', theme: 'dark', locale: 'it' });
    await expect(page).toHaveScreenshot('welcome-appearance-satin-dark.png', { animations: 'disabled' });
  });
});

test('attention states say why a request cannot be decided, and show hidden characters', async ({ page }) => {
  await open(page, { view: 'attention' });
  const card = (name: string) => page.getByRole('region', { name }).filter({ has: page.getByLabel('Command') });
  const command = page.getByRole('region', { name: 'Claude wants to run a command' });
  await expect(command.first().getByText('Expires in 4:32')).toBeVisible();
  await expect(page.getByText('This request has expired.')).toBeVisible();
  await expect(page.getByText(/The agent working on “Write the tests” has stopped/)).toBeVisible();
  await expect(page.getByText(/Lumen cannot approve this: This tool requires an adapter/)).toBeVisible();
  await expect(page.getByRole('region', { name: /cannot approve/ }).getByRole('button', { name: 'Approve once' })).toBeDisabled();
  await expect(page.getByRole('alert').filter({ hasText: 'could not confirm that the agent received' })).toBeVisible();
  await expect(page.getByText('Expired', { exact: true })).toBeVisible();
  // The right-to-left override in the command shows as an escape, not as reversed text.
  await expect(card('Claude wants to run a command').getByLabel('Command').filter({ hasText: 'echo safe' })).toHaveText(String.raw`echo safe\u202e && del *.txt`);
  // Only Codex can be stopped from its card; its diff is shown.
  await expect(page.getByRole('button', { name: 'Stop the turn' })).toHaveCount(1);
  await expect(page.getByRole('region', { name: 'Codex wants to change files' })).toContainText('LIVE_LIMIT');
  await expect(page.getByLabel('Your answer (not saved)')).toHaveAttribute('type', 'password');
  await expect(page.getByText('Lumen could not check whether the agent is still waiting.')).toBeVisible();
});

test('a question card works from the keyboard', async ({ page }) => {
  await open(page, { view: 'attention' });
  const form = page.getByRole('form', { name: 'The agent has questions' });
  const send = form.getByRole('button', { name: 'Send answers' });
  await expect(send).toBeDisabled();
  await form.getByRole('radio', { name: /Native/ }).focus();
  await page.keyboard.press('Space');
  await expect(form.getByRole('radio', { name: /Native/ })).toBeChecked();
  await page.keyboard.press('ArrowDown');
  await expect(form.getByRole('radio', { name: /Custom/ })).toBeChecked();
  await form.getByLabel('Your answer (not saved)').fill('token');
  await expect(send).toBeEnabled();
  // A typed answer replaces the chosen option of a single-answer question.
  await form.getByLabel('A different answer').first().fill('Something else');
  await expect(form.getByRole('radio', { name: /Custom/ })).not.toBeChecked();
});
