// M6 against the real daemon: accounts in Settings. CLI accounts sign in through their official
// app (offline fixtures here), quota that was never reported shows as such, and an API key is
// write-only: it reaches the OS keychain and never the page or the store.
import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { startDaemon } from './daemon-fixture';

test.describe.configure({ mode: 'serial' });

let daemon: Awaited<ReturnType<typeof startDaemon>>;
test.beforeAll(async ({ baseURL }) => {
  daemon = await startDaemon(new URL(baseURL!).origin);
});
test.afterAll(async () => {
  // Removing an API account also deletes its key from the OS keychain.
  const accounts = (await daemon?.client.call('list_accounts').catch(() => [])) as { id: string }[];
  for (const a of accounts ?? []) await daemon.client.call('remove_account', { id: a.id }).catch(() => {});
  await daemon?.close();
});

const settings = (page: Page) => page.getByRole('region', { name: 'Settings' });
async function open(page: Page) {
  await page.goto(daemon.url());
  await page.getByRole('button', { name: 'Settings' }).click();
  await expect(settings(page).getByRole('heading', { name: 'Accounts' })).toBeVisible();
}
async function add(page: Page, agent: string, name?: string) {
  await settings(page).getByRole('button', { name: 'Add account' }).click();
  const dialog = page.getByRole('alertdialog');
  await dialog.getByRole('button', { name: /Agent/ }).click();
  await page.getByRole('option', { name: agent, exact: true }).click();
  if (name) await dialog.getByRole('textbox', { name: 'Name' }).fill(name);
  await dialog.getByRole('button', { name: 'Add account' }).click();
}
async function files(dir: string): Promise<string[]> {
  const out: string[] = [];
  for (const e of await readdir(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) out.push(...(await files(p)));
    else out.push(p);
  }
  return out;
}

test('an unavailable emergency pin can be replaced with Auto', async ({ page }) => {
  const value = await daemon.client.call('get_settings') as Record<string, unknown>;
  await daemon.client.call('save_settings', { value: { ...value, emergencyModels: { opencode: 'opencode-go/removed-model' } } });
  await open(page);
  await add(page, 'OpenCode');
  const card = settings(page).getByRole('listitem', { name: 'OpenCode, OpenCode' });
  await expect(card.getByRole('alert')).toContainText('selected emergency model is unavailable');
  await card.getByRole('button', { name: /Emergency model/ }).click();
  await page.getByRole('option', { name: 'Auto', exact: true }).click();
  await expect(card.getByRole('alert')).toHaveCount(0);
  await expect.poll(async () => ((await daemon.client.call('get_settings')) as { emergencyModels: Record<string, string> }).emergencyModels.opencode).toBeUndefined();
  await card.getByRole('button', { name: 'More actions' }).click();
  await page.getByRole('menuitem', { name: 'Remove' }).click();
  await page.getByRole('alertdialog').getByRole('button', { name: 'Remove' }).click();
});

test('a CLI account is added, checked and shows quota as not reported', async ({ page }) => {
  await open(page);
  await expect(settings(page)).toContainText('No accounts yet');
  await add(page, 'Claude Code');
  const card = settings(page).getByRole('listitem', { name: 'Claude Code, Claude Code' });
  const plans = page.getByRole('alertdialog', { name: 'Models included in Claude Code' });
  const fable = plans.getByRole('switch', { name: 'Includes Fable' });
  const mythos = plans.getByRole('switch', { name: 'Includes Mythos' });
  await expect(plans).toBeVisible();
  await expect(fable).not.toBeChecked();
  await expect(mythos).not.toBeChecked();
  const [account] = await daemon.client.call('list_accounts') as { id: string }[];
  const catalog = async () => (await daemon.client.call('list_models', { id: account.id }) as { id: string }[]).map((m) => m.id);
  expect(await catalog()).not.toContain('fable');
  await fable.press('Space');
  await plans.getByRole('button', { name: 'Save' }).click();
  await expect.poll(catalog).toContain('fable');
  expect(await catalog()).not.toContain('claude-mythos-5-2');
  // The fixture CLI reports a signed-in account and no usage: absent, never an empty bar.
  await expect(card.getByText('Connected')).toBeVisible();
  await expect(card).toContainText('Usage not reported yet');
  await expect(card.getByRole('meter')).toHaveCount(0);
  await card.getByText('What it can do').click();
  await expect(card).toContainText('Resume its own session');
  const { violations } = await new AxeBuilder({ page }).withTags(['wcag2a', 'wcag2aa', 'wcag21aa', 'wcag22aa']).analyze();
  expect(violations.filter((v) => v.impact === 'serious' || v.impact === 'critical').map((v) => v.id)).toEqual([]);
});

test('an account is renamed and removed with confirmation', async ({ page }) => {
  await daemon.stop();
  await daemon.restart();
  await open(page);
  const card = settings(page).getByRole('listitem', { name: /^Claude Code,/ });
  await card.getByRole('button', { name: 'More actions' }).click();
  await page.getByRole('menuitem', { name: 'Models included in the plan' }).click();
  await expect(page.getByRole('switch', { name: 'Includes Fable' })).toBeChecked();
  await expect(page.getByRole('switch', { name: 'Includes Mythos' })).not.toBeChecked();
  await page.getByRole('alertdialog').getByRole('button', { name: 'Cancel' }).click();
  await card.getByRole('button', { name: 'More actions' }).click();
  await page.getByRole('menuitem', { name: 'Rename' }).click();
  await page.getByRole('alertdialog').getByRole('textbox', { name: 'Name' }).fill('Work');
  await page.getByRole('alertdialog').getByRole('button', { name: 'Save' }).click();
  const renamed = settings(page).getByRole('listitem', { name: 'Work, Claude Code' });
  await expect(renamed).toBeVisible();
  await renamed.getByRole('button', { name: 'More actions' }).click();
  await page.getByRole('menuitem', { name: 'Remove' }).click();
  await expect(page.getByRole('alertdialog')).toContainText('Remove Work?');
  await page.getByRole('alertdialog').getByRole('button', { name: 'Remove' }).click();
  await expect(settings(page)).toContainText('No accounts yet');
  expect(await daemon.client.call('list_accounts')).toEqual([]);
});

test('an API key is write-only and Auto needs a budget', async ({ page }) => {
  const secret = `sk-lumen-fixture-${Date.now()}-never-shown`;
  await open(page);
  await add(page, 'Anthropic API', 'Lab');
  const card = settings(page).getByRole('listitem', { name: 'Lab, Anthropic API' });
  const auto = card.getByRole('switch', { name: /Let Auto use this account/ });
  await expect(auto).toBeDisabled();
  const key = card.getByLabel('API key');
  await key.fill(secret);
  await card.getByRole('button', { name: 'Save key' }).click();
  await expect(card).toContainText('A key is saved in the system keychain.');
  await expect(key).toHaveValue('');
  await card.getByRole('textbox', { name: 'Model ID' }).fill('claude-fixture-1');
  await card.getByRole('textbox', { name: 'Input, USD per million tokens' }).fill('3');
  await card.getByRole('textbox', { name: 'Output, USD per million tokens' }).fill('15');
  await card.getByRole('textbox', { name: 'Budget (USD)' }).fill('5');
  const save = card.getByRole('button', { name: 'Save', exact: true });
  // The rates must be confirmed by the user, never assumed.
  await expect(save).toBeDisabled();
  await card.getByRole('switch', { name: 'These are the official rates for this model' }).check({ force: true });
  await save.click();
  await expect(card.getByRole('status')).toHaveText('Saved');
  await expect(auto).toBeEnabled();
  // The switch follows the daemon: it turns on once the setting is saved.
  await card.getByText('Let Auto use this account').click();
  await expect(auto).toBeChecked();
  const [account] = (await daemon.client.call('list_accounts')) as { id: string }[];
  await expect.poll(async () => ((await daemon.client.call('get_api_settings', { id: account.id })) as { config: { autoEnabled: boolean } }).config.autoEnabled).toBe(true);
  // The key is nowhere the page or the store can reach.
  expect(await page.content()).not.toContain(secret);
  expect(JSON.stringify(await daemon.client.call('get_api_settings', { id: account.id }))).not.toContain(secret);
  // Every file the service wrote, read once it has stopped and released them.
  await daemon.stop();
  for (const f of await files(daemon.data)) expect((await readFile(f)).includes(secret), f).toBe(false);
  await daemon.restart();
});
