// M4 against the real daemon and the offline CLI fixtures: Gatekeeper approvals (approve, reject,
// stop the turn) and native questions, including a secret answer that must leave no trace.
import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { startDaemon } from './daemon-fixture';

test.skip(process.platform !== 'win32', 'CLI fixtures are Windows-only');
test.describe.configure({ mode: 'serial' });

const CLAUDE = 'Claude fixture';
const CODEX = 'Codex fixture';
const SECRET = 'fixture-secret-value';
let daemon: Awaited<ReturnType<typeof startDaemon>>;
test.beforeAll(async ({ baseURL }) => {
  daemon = await startDaemon(new URL(baseURL!).origin);
  await daemon.client.call('add_account', { name: CLAUDE, provider: 'claude' });
  await daemon.client.call('add_account', { name: CODEX, provider: 'gpt' });
});
test.afterAll(async () => daemon?.close());

/** A fresh conversation, opened with the requested access and account. */
async function open(page: Page, account: string | null, mode = 'ask') {
  const chat = (await daemon.client.call('create_session', { workspace: daemon.workspace })) as { id: string };
  await daemon.client.call('set_session_access_mode', { id: chat.id, mode });
  await page.goto(daemon.url());
  await page.getByRole('navigation').getByRole('listitem').first().getByRole('button').first().click();
  if (account) {
    await page.getByRole('button', { name: /Agent/ }).click();
    await page.getByRole('option', { name: account }).click();
    await expect(page.getByRole('button', { name: /Model/ })).toBeVisible();
  }
  return chat;
}

async function send(page: Page, text: string) {
  const box = page.getByRole('textbox', { name: 'Describe what you want to do' });
  await box.fill(text);
  await box.press('Enter');
}

const attention = (page: Page) => page.getByRole('region', { name: 'Needs your attention' });
const reply = (page: Page) => page.getByRole('log').getByRole('article').last();

async function seriousViolations(page: Page) {
  const { violations } = await new AxeBuilder({ page }).include('main').withTags(['wcag2a', 'wcag2aa', 'wcag21aa', 'wcag22aa']).analyze();
  return violations.filter((v) => v.impact === 'serious' || v.impact === 'critical').map((v) => v.id);
}

test('an approved command reaches the agent once, and the card says what it approves', async ({ page }) => {
  test.setTimeout(90_000);
  await open(page, CLAUDE);
  await send(page, 'APPROVAL_FIXTURE approve');
  const card = attention(page).getByRole('region', { name: 'Claude wants to run a command' });
  await expect(card).toBeVisible({ timeout: 30_000 });
  await expect(card.getByLabel('Command')).toHaveText('fixture-only: git status --short');
  await expect(card.getByText(/Expires in [45]:[0-9]{2}/)).toBeVisible();
  expect(await seriousViolations(page)).toEqual([]);
  await card.getByRole('button', { name: 'Approve once' }).click();
  await expect(reply(page).getByText('APPROVAL_FIXTURE_ALLOW')).toBeVisible({ timeout: 30_000 });
  await expect(attention(page)).toHaveCount(0);
});

test('a rejected command is denied, with the note going to the agent', async ({ page }) => {
  test.setTimeout(90_000);
  const chat = await open(page, CLAUDE);
  await send(page, 'APPROVAL_FIXTURE reject');
  const card = attention(page).getByRole('region', { name: 'Claude wants to run a command' });
  await expect(card).toBeVisible({ timeout: 30_000 });
  // Claude has no separate cancel: it would only deny, like Reject.
  await expect(card.getByRole('button', { name: 'Stop the turn' })).toHaveCount(0);
  await card.getByRole('textbox', { name: 'Note for the agent (optional)' }).fill('Not now');
  await card.getByRole('button', { name: 'Reject' }).click();
  await expect(reply(page).getByText('APPROVAL_FIXTURE_DENY')).toBeVisible({ timeout: 30_000 });
  const snapshot = (await daemon.client.call('gatekeeper_snapshot', { chat: chat.id })) as { requests: { status: string; note: string }[] };
  expect(snapshot.requests.map((r) => [r.status, r.note])).toEqual([['rejected', 'Not now']]);
});

test('Codex can be stopped from its approval card', async ({ page }) => {
  test.setTimeout(90_000);
  const chat = await open(page, CODEX);
  await send(page, 'APPROVAL_FIXTURE stop');
  const card = attention(page).getByRole('region', { name: 'Codex wants to run a command' });
  await expect(card).toBeVisible({ timeout: 30_000 });
  await card.getByRole('button', { name: 'Stop the turn' }).click();
  await expect(card).toHaveCount(0, { timeout: 30_000 });
  // The stopped turn is not continued by itself.
  await expect(attention(page).getByText(/This reply was interrupted|Lumen kept what it had written/)).toBeVisible();
  const snapshot = (await daemon.client.call('gatekeeper_snapshot', { chat: chat.id })) as { requests: { status: string }[] };
  expect(snapshot.requests.map((r) => r.status)).toEqual(['cancelled']);
});

test('a native question is answered from its card and the agent continues', async ({ page }) => {
  test.setTimeout(90_000);
  await open(page, CLAUDE);
  await send(page, 'QUESTION_FIXTURE');
  const card = attention(page).getByRole('form', { name: 'The agent has a question' });
  await expect(card).toBeVisible({ timeout: 30_000 });
  const send_ = card.getByRole('button', { name: 'Send answers' });
  await expect(send_).toBeDisabled();
  await card.getByText('Native', { exact: true }).click();
  await expect(card.getByRole('radio', { name: /Native/ })).toBeChecked();
  expect(await seriousViolations(page)).toEqual([]);
  await send_.click();
  await expect(reply(page).getByRole('heading', { name: 'A small, thoughtful plan' })).toBeVisible({ timeout: 30_000 });
  await expect(attention(page)).toHaveCount(0);
  expect((await readFile(path.join(daemon.workspace, 'answer-claude.txt'), 'utf8')).trim()).toBe('Native');
});

test('Codex Autopilot waits for an interactive choice and resumes the turn', async ({ page }) => {
  test.setTimeout(90_000);
  const chat = await open(page, null, 'autopilot');
  await send(page, 'Use Codex: research online sources and compare routing approaches. AUTOPILOT_FIXTURE_DECISION');
  const card = attention(page).getByRole('form', { name: 'The agent has a question' });
  await expect(card).toBeVisible({ timeout: 30_000 });
  const header = page.locator('main > header');
  await expect(header).toContainText('Waiting');
  expect(JSON.stringify(await daemon.client.call('get_messages', { id: chat.id }))).toContain('GPT Sol fixture · high');
  await expect(card.getByRole('radio')).toHaveCount(2);
  await expect(card.getByRole('button', { name: 'Send answers' })).toBeDisabled();
  await card.getByText('Quality', { exact: true }).click();
  await card.getByRole('button', { name: 'Send answers' }).click();
  await expect(reply(page).getByText('Priority confirmed.')).toBeVisible({ timeout: 30_000 });
  await expect(header).toContainText('Done');
  await expect(attention(page)).toHaveCount(0);
});

test('a secret answer reaches the agent and is kept nowhere else', async ({ page }) => {
  test.setTimeout(90_000);
  const chat = await open(page, CODEX);
  await send(page, 'SECRET_ASK_FIXTURE');
  const card = attention(page).getByRole('form', { name: 'The agent has a question' });
  await expect(card).toBeVisible({ timeout: 30_000 });
  const field = card.getByLabel('Your answer (not saved)');
  await expect(field).toHaveAttribute('type', 'password');
  await field.fill(SECRET);
  await card.getByRole('button', { name: 'Send answers' }).click();
  await expect(reply(page).getByText('SECRET_QUESTION_RECEIVED')).toBeVisible({ timeout: 30_000 });
  // Not in the page, its storage, the daemon's transcript or anything in its data directory.
  expect(await page.content()).not.toContain(SECRET);
  // page.content() shows attributes, not what a field holds: read every field's live value.
  expect(await page.evaluate(() => [...document.querySelectorAll('input, textarea')].map((e) => (e as HTMLInputElement).value).join('\n'))).not.toContain(SECRET);
  expect(await page.evaluate(() => JSON.stringify({ ...localStorage }) + JSON.stringify({ ...sessionStorage }))).not.toContain(SECRET);
  expect(JSON.stringify(await daemon.client.call('get_messages', { id: chat.id }))).not.toContain(SECRET);
  for (const file of await readdir(daemon.data, { recursive: true, withFileTypes: true })) {
    if (!file.isFile()) continue;
    const bytes = await readFile(path.join(file.parentPath, file.name)).catch(() => null);
    expect(bytes?.includes(SECRET) ?? false, `${file.name} holds the secret`).toBe(false);
  }
});

test('a reply stopped by the quota resumes once, on request, when the quota resets', async ({ page }) => {
  test.setTimeout(90_000);
  await open(page, CLAUDE);
  await send(page, 'QUOTA_RESET_FIXTURE');
  const resume = attention(page).getByRole('button', { name: /Resume when the quota resets/ });
  await expect(resume).toBeVisible({ timeout: 30_000 });
  // Nothing resumed by itself.
  expect((await readFile(path.join(daemon.workspace, 'quota-reset-attempts.txt'), 'utf8')).trim().split(/\r?\n/)).toEqual(['initial']);
  await resume.click();
  await expect(page.getByRole('log').getByText('RESUME_AFTER_QUOTA_OK')).toBeVisible({ timeout: 30_000 });
  expect((await readFile(path.join(daemon.workspace, 'quota-reset-attempts.txt'), 'utf8')).trim().split(/\r?\n/)).toEqual(['initial', 'resume']);
  await expect(resume).toHaveCount(0);
});

test('rewind shows what it undoes and gives the prompt back to edit', async ({ page }) => {
  test.setTimeout(90_000);
  await open(page, CLAUDE);
  await send(page, 'Rewind me');
  await expect(reply(page).getByRole('heading', { name: 'A small, thoughtful plan' })).toBeVisible({ timeout: 30_000 });
  const mine = page.getByRole('log').getByRole('article').first();
  await mine.hover();
  await mine.getByRole('button', { name: 'Rewind to here' }).click();
  const dialog = page.getByRole('alertdialog', { name: 'Rewind the conversation?' });
  await expect(dialog).toContainText('This message and the 1 after it are removed.');
  await expect(dialog).toContainText('Files in the project are not changed.');
  expect(await seriousViolations(page)).toEqual([]);
  await dialog.getByRole('button', { name: 'Rewind' }).click();
  await expect(page.getByText('No messages yet')).toBeVisible();
  await expect(page.getByRole('textbox', { name: 'Describe what you want to do' })).toHaveValue('Rewind me');
});

test('a conversation continues in a new one seeded from it', async ({ page }) => {
  test.setTimeout(90_000);
  await open(page, CLAUDE);
  await send(page, 'Compact me');
  await expect(reply(page).getByRole('heading', { name: 'A small, thoughtful plan' })).toBeVisible({ timeout: 30_000 });
  const list = async () => (await daemon.client.call('list_sessions', { search: '', offset: 0 })) as { id: string; title: string }[];
  const before = new Set((await list()).map((x) => x.id));
  await page.locator('main header').getByRole('button', { name: 'More actions' }).click();
  await page.getByRole('menuitem', { name: 'Continue in a new conversation' }).click();
  await expect.poll(async () => (await list()).filter((x) => !before.has(x.id)).length).toBe(1);
  const created = (await list()).find((x) => !before.has(x.id))!;
  // The new conversation is the one now open; the old one is still there.
  await expect(page.locator('main header h1')).toHaveText(created.title || 'New conversation');
  expect((await list()).some((x) => x.title === 'Compact me')).toBe(true);
});
