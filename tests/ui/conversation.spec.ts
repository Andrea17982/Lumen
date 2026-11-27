// M3 against the real daemon and the offline Claude CLI fixture: send from the composer, stream
// to a finished Markdown reply, Stop, and shortened tool output with "Show all".
import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { sleep } from '../daemon-harness.mjs';
import { startDaemon } from './daemon-fixture';

test.skip(process.platform !== 'win32', 'CLI fixtures are Windows-only');
test.describe.configure({ mode: 'serial' });

let daemon: Awaited<ReturnType<typeof startDaemon>>;
let accountName = 'UI fixture';
test.beforeAll(async ({ baseURL }) => {
  daemon = await startDaemon(new URL(baseURL!).origin);
  await daemon.client.call('add_account', { name: accountName, provider: 'claude' });
});
test.afterAll(async () => daemon?.close());

/** A fresh conversation, opened, with the fixture account chosen as the agent. */
async function open(page: Page) {
  const chat = await daemon.client.call('create_session', { workspace: daemon.workspace });
  await page.goto(daemon.url());
  await page.getByRole('navigation').getByRole('listitem').first().getByRole('button').first().click();
  // As in attention.spec: a press while the list is still opening may not take; choose again.
  const agent = page.getByRole('button', { name: /Agent/ });
  await expect(async () => {
    if (!(await agent.textContent())?.includes(accountName)) {
      await agent.click();
      await page.getByRole('option', { name: accountName }).click({ timeout: 5_000 });
    }
    await expect(page.getByRole('button', { name: /Model/ })).toBeVisible({ timeout: 5_000 });
  }).toPass({ timeout: 30_000 });
  return chat;
}

async function send(page: Page, text: string) {
  const box = page.getByRole('textbox', { name: 'Describe what you want to do' });
  await box.fill(text);
  await box.press('Enter');
}

test('model search filters and selects with the keyboard', async ({ page }) => {
  await open(page);
  const model = page.getByRole('combobox', { name: 'Model' });
  await model.fill('haiku');
  await expect(page.getByRole('option', { name: /haiku/i }).first()).toBeVisible();
  await expect(page.getByRole('option', { name: /sonnet/i })).toHaveCount(0);
  await model.press('ArrowDown');
  await model.press('Enter');
  await expect(model).toHaveValue(/haiku/i);
});

test('a fixture turn goes from prompt to a finished Markdown reply', async ({ page }) => {
  test.setTimeout(90_000);
  await open(page);
  await send(page, 'Hello from the composer');
  const log = page.getByRole('log');
  await expect(log.getByText('Hello from the composer')).toBeVisible();
  const reply = log.getByRole('article').last();
  await expect(reply.getByRole('heading', { name: 'A small, thoughtful plan' })).toBeVisible({ timeout: 30_000 });
  await expect(reply.getByRole('cell', { name: 'Understand' })).toBeVisible();
  await expect(reply.locator('code', { hasText: 'const clarity = true;' })).toBeVisible();
  await expect(reply.getByRole('strong')).toHaveText('offline UI fixture');
  await expect(page.locator('main header')).toContainText('Done');
  await reply.getByText('1 step').click();
  await expect(reply.getByText('Read sample.ts')).toBeVisible();
  await expect(reply.getByText(/Why this agent: Native session/)).toBeVisible();
  // One user message and one reply: the optimistic bubble gave way to the daemon's message.
  await expect(log.getByRole('article')).toHaveCount(2);
  const { violations } = await new AxeBuilder({ page }).include('main').withTags(['wcag2a', 'wcag2aa', 'wcag21aa', 'wcag22aa']).analyze();
  expect(violations.filter((v) => v.impact === 'serious' || v.impact === 'critical').map((v) => v.id)).toEqual([]);
});

test('Stop cancels a running turn and the daemon reports it', async ({ page }) => {
  test.setTimeout(90_000);
  await open(page);
  await send(page, 'SLOW_FIXTURE please wait');
  await expect(page.locator('main header')).toContainText('Running', { timeout: 15_000 });
  await sleep(500);
  await page.getByRole('button', { name: 'Stop' }).click();
  await expect(page.locator('main header')).toContainText(/Interrupted|Stopped/, { timeout: 15_000 });
  await expect(page.getByRole('button', { name: 'Send' })).toBeVisible();
});

test('a shortened tool output shows everything on request', async ({ page }) => {
  test.setTimeout(90_000);
  await open(page);
  await send(page, 'LARGE_TOOL_FIXTURE');
  const reply = page.getByRole('log').getByRole('article').last();
  await expect(reply.getByText('Large tool fixture done.')).toBeVisible({ timeout: 30_000 });
  await reply.getByText('1 step').click();
  await reply.locator('ol summary').first().click();
  await expect(reply.getByText(/Shortened: [0-9]+ lines? in total/)).toBeVisible();
  await expect(reply.getByText(/LARGE_TOOL_END/)).toHaveCount(0);
  // The turn is over: the full text comes from the saved message (kept up to 1 MiB).
  await reply.getByRole('button', { name: 'Show all' }).click();
  await expect(reply.getByText(/LARGE_TOOL_END/)).toBeVisible();
  await expect(reply.getByRole('button', { name: 'Show all' })).toHaveCount(0);
});
