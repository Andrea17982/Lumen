// M8 against the real daemon: General settings, extensions (MCP, skills, a plugin with a sandboxed
// panel), /every, notes on a reply and the notification when a reply is ready.
import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { startDaemon } from './daemon-fixture';

test.skip(process.platform !== 'win32', 'CLI fixtures are Windows-only');
test.describe.configure({ mode: 'serial' });

let daemon: Awaited<ReturnType<typeof startDaemon>>;
let skill: string;
let plugin: string;
test.beforeAll(async ({ baseURL }) => {
  daemon = await startDaemon(new URL(baseURL!).origin);
  const account = await daemon.client.call('add_account', { name: 'Claude fixture', provider: 'claude' }) as { id: string };
  await daemon.client.call('set_claude_models', { id: account.id, value: { fable: false, mythos: false } });
  skill = path.join(daemon.data, 'fixture-skill');
  await mkdir(skill);
  await writeFile(path.join(skill, 'SKILL.md'), '---\nname: fixture-skill\ndescription: A test skill.\n---\nDo nothing.\n');
  // A plugin whose panel tries to leave its sandbox: a direct request and an undeclared host.
  // Plugins must come from outside Lumen's data folder.
  plugin = await mkdtemp(path.join(tmpdir(), 'lumen-plugin-'));
  await writeFile(
    path.join(plugin, 'lumen-plugin.json'),
    JSON.stringify({ id: 'panel-fixture', name: 'Panel fixture', version: '1.0.0', description: 'Tests the panel sandbox.', lumen: '>=0.10.0 <1.0.0', permissions: { network: ['example.org'] }, contributes: { panels: [{ id: 'main', title: 'Probe', entry: 'panel.html' }] } }),
  );
  await writeFile(
    path.join(plugin, 'panel.html'),
    `<!doctype html><p>Probe</p><script>
const ask = (id, method, params) => new Promise((resolve) => {
  addEventListener('message', function on(e) { if (e.data && e.data.id === id) { removeEventListener('message', on); resolve(e.data); } });
  parent.postMessage({ lumen: 1, id, method, params }, '*');
});
(async () => {
  let direct = 'FETCH_BLOCKED';
  try { await fetch('http://127.0.0.1:9/'); direct = 'FETCH_ALLOWED'; } catch {}
  let storage = 'STORAGE_BLOCKED';
  try { localStorage.getItem('x'); storage = 'STORAGE_ALLOWED'; } catch {}
  const broker = await ask(1, 'net.fetch', { url: 'https://undeclared.example.com/' });
  const other = await ask(2, 'shell.run', {});
  await ask(3, 'prompt.insert', { text: [direct, storage, broker.error ? 'BROKER_REFUSED' : 'BROKER_ALLOWED', other.error ? 'METHOD_REFUSED' : 'METHOD_ALLOWED', 'HIDDEN\u200bTEXT'].join(' ') });
})();
</script>`,
  );
});
test.afterAll(async () => {
  await daemon?.close();
  if (plugin) await rm(plugin, { recursive: true, force: true });
});

const settings = (page: Page) => page.getByRole('region', { name: 'Settings' });
async function openSettings(page: Page, section: string) {
  await page.goto(daemon.url());
  await page.getByRole('button', { name: 'Settings' }).click();
  await settings(page).getByRole('radio', { name: section }).click({ force: true });
  await expect(settings(page).getByRole('heading', { name: section })).toBeVisible();
}
async function seriousViolations(page: Page) {
  const { violations } = await new AxeBuilder({ page }).include('main').withTags(['wcag2a', 'wcag2aa', 'wcag21aa', 'wcag22aa']).analyze();
  return violations.filter((v) => v.impact === 'serious' || v.impact === 'critical').map((v) => v.id);
}
type Settings = { orchestra: boolean; codeMode: string; closeToTray: boolean; codeModeChosen: boolean };
const daemonSettings = async () => (await daemon.client.call('get_settings')) as Settings;

test('General has few choices, each saved where it belongs', async ({ page }) => {
  await openSettings(page, 'General');
  // Code Mode is a plain switch over the daemon's setting.
  const code = settings(page).getByRole('switch', { name: /Code Mode/ });
  await expect(code).toBeChecked();
  await code.click({ force: true });
  await expect(code).not.toBeChecked();
  expect(await daemonSettings()).toMatchObject({ codeMode: 'off', codeModeChosen: true });
  const tray = settings(page).getByRole('switch', { name: /notification area/ });
  const before = (await daemonSettings()).closeToTray;
  await tray.click({ force: true });
  await expect.poll(async () => (await daemonSettings()).closeToTray).toBe(!before);
  // Style and language are this window's: no daemon round trip.
  await settings(page).getByRole('radio', { name: 'Satin' }).click({ force: true });
  await expect(page.locator('html')).toHaveAttribute('data-style', 'satin');
  await settings(page).getByRole('radio', { name: 'Ink' }).click({ force: true });
  await expect(page.locator('html')).toHaveAttribute('data-style', 'ink');
  // Restore defaults asks first, then puts the daemon's defaults back.
  await settings(page).getByRole('button', { name: 'Restore defaults' }).click();
  await page.getByRole('alertdialog').getByRole('button', { name: 'Restore defaults' }).click();
  await expect(code).toBeChecked();
  expect((await daemonSettings()).codeMode).toBe('auto');
  await settings(page).getByText('About').click();
  await expect(settings(page)).toContainText(/Lumen \d+\.\d+\.\d+/);
  expect(await seriousViolations(page)).toEqual([]);
});

test('the language follows the choice, without a reload', async ({ page }) => {
  // No locale in the URL: the saved choice applies.
  await page.goto(daemon.url().replace('locale=en&', '').replace(/[?&]locale=en/, ''));
  await page.getByRole('button', { name: /Settings|Impostazioni/ }).click();
  await settings(page).or(page.getByRole('region', { name: 'Impostazioni' })).getByRole('radio', { name: /General|Generali/ }).click({ force: true });
  await page.getByRole('button', { name: /Language|Lingua/ }).click();
  await page.getByRole('option', { name: 'Italiano' }).click();
  await expect(page.getByRole('region', { name: 'Impostazioni' })).toBeVisible();
  await page.getByRole('button', { name: /Lingua/ }).click();
  await page.getByRole('option', { name: 'English' }).click();
  await expect(settings(page)).toBeVisible();
});

test('MCP servers and skills are added, switched off and removed', async ({ page }) => {
  await openSettings(page, 'Extensions');
  await settings(page).getByRole('textbox', { name: 'Name' }).fill('fixture-memory');
  await settings(page).getByRole('textbox', { name: 'Command or URL' }).fill('https://mcp.example.com/sse');
  await settings(page).getByRole('button', { name: 'Add server' }).click();
  const row = settings(page).getByRole('listitem', { name: 'fixture-memory' });
  await expect(row).toContainText('Ready');
  await row.getByRole('switch').click({ force: true });
  await expect(row.getByRole('switch')).not.toBeChecked();
  expect(((await daemon.client.call('mcp_health')) as { name: string; enabled: boolean }[]).find((s) => s.name === 'fixture-memory')?.enabled).toBe(false);
  await row.getByRole('button', { name: 'More actions' }).click();
  await page.getByRole('menuitem', { name: 'Remove' }).click();
  await page.getByRole('alertdialog').getByRole('button', { name: 'Remove' }).click();
  await expect(row).toHaveCount(0);

  page.once('dialog', (d) => d.accept(skill));
  await settings(page).getByRole('button', { name: 'Add a skill folder…' }).click();
  await expect(settings(page)).toContainText('fixture-skill');
  expect(await seriousViolations(page)).toEqual([]);
});

test('a plugin is installed after showing what it asks, and its panel stays in its sandbox', async ({ page }) => {
  // The composer lives in a conversation: open one first, then go to Settings from it.
  const chat = (await daemon.client.call('create_session', { workspace: daemon.workspace })) as { id: string };
  await page.goto(daemon.url());
  await page.getByRole('navigation').getByRole('listitem').first().getByRole('button').first().click();
  await page.getByRole('button', { name: 'Settings' }).click();
  await settings(page).getByRole('radio', { name: 'Extensions' }).click({ force: true });
  page.once('dialog', (d) => d.accept(plugin));
  await settings(page).getByRole('button', { name: 'Add a plugin folder…' }).click();
  const dialog = page.getByRole('alertdialog');
  await expect(dialog).toContainText('Install Panel fixture?');
  await expect(dialog).toContainText('Connect to example.org');
  await expect(dialog).toContainText('1 panel');
  await dialog.getByRole('button', { name: 'Install' }).click();
  const card = settings(page).getByRole('listitem', { name: 'Panel fixture' });
  await expect(card.getByRole('switch', { name: 'On' })).toBeChecked();
  await card.getByRole('button', { name: 'Open Probe' }).click();
  await expect(card.getByRole('status')).toHaveText('Added to the message box.', { timeout: 15_000 });
  await settings(page).getByRole('button', { name: 'Back to the conversation' }).click();
  await expect(page.getByRole('textbox', { name: 'Describe what you want to do' })).toHaveValue(String.raw`FETCH_BLOCKED STORAGE_BLOCKED BROKER_REFUSED METHOD_REFUSED HIDDEN\u200bTEXT`);
  expect(chat.id).toBeTruthy();
});

test('/every schedules a recurring task that can be stopped from the conversation menu', async ({ page }) => {
  await daemon.client.call('create_session', { workspace: daemon.workspace });
  await page.goto(daemon.url());
  await page.getByRole('navigation').getByRole('listitem').first().getByRole('button').first().click();
  const box = page.getByRole('textbox', { name: 'Describe what you want to do' });
  await box.fill('/');
  await expect(page.getByRole('list', { name: 'Commands' })).toContainText('/every');
  await box.fill('/every');
  await box.press('Enter');
  await expect(box).toHaveValue('/every ');
  await box.press('Enter');
  await expect(page.getByRole('alert')).toContainText('Use /every <interval> <task>');
  await box.fill('/every 1d check outdated dependencies');
  await box.press('Enter');
  await expect(page.getByRole('status').filter({ hasText: 'Recurring task scheduled.' })).toBeVisible();
  await page.locator('main header').getByRole('button', { name: 'More actions' }).click();
  await page.getByRole('menuitem', { name: 'Recurring tasks' }).click();
  const panel = page.getByRole('complementary').filter({ hasText: 'Recurring tasks' });
  await expect(panel).toContainText('Every 1d');
  await expect(panel).toContainText('check outdated dependencies');
  await panel.getByRole('button', { name: 'Stop' }).click();
  await expect(panel.getByText('check outdated dependencies')).toHaveCount(0);
});

test('a finished reply notifies, and can be kept as a recipe', async ({ page }) => {
  await daemon.client.call('create_session', { workspace: daemon.workspace });
  await page.goto(daemon.url());
  await page.getByRole('navigation').getByRole('listitem').first().getByRole('button').first().click();
  const box = page.getByRole('textbox', { name: 'Describe what you want to do' });
  await box.fill('Say hello');
  await box.press('Enter');
  const reply = page.getByRole('log').getByRole('article').last();
  await expect(reply.getByRole('button', { name: 'Save as recipe' })).toBeAttached({ timeout: 30_000 });
  await expect.poll(() => page.evaluate(() => (window as unknown as { __lumenNotifications?: { title: string }[] }).__lumenNotifications?.map((n) => n.title) ?? [])).toContain('Reply ready');
  await reply.hover();
  await reply.getByRole('button', { name: 'Save as recipe' }).click();
  await expect(reply).toContainText('Saved as recipe');
});
