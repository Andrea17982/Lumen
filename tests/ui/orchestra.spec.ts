// M7 against the real daemon and the offline CLI fixtures: Orchestra as one switch in Settings,
// the Git offer, questions, the plan with edits, live lanes, cancel, and resume after a restart.
import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';
import { existsSync } from 'node:fs';
import path from 'node:path';
import { startDaemon } from './daemon-fixture';

test.skip(process.platform !== 'win32', 'CLI fixtures are Windows-only');
test.describe.configure({ mode: 'serial' });

// Two areas and "build": the daemon's own rule makes this a complex task. The marker makes the
// fixture workers stream a partial line and take a few seconds, so a lane can be watched live.
const TASK = 'Build the backend API and the frontend UI as two files. DAEMON_ORCHESTRA_FIXTURE';
let daemon: Awaited<ReturnType<typeof startDaemon>>;
test.beforeAll(async ({ baseURL }) => {
  daemon = await startDaemon(new URL(baseURL!).origin);
  await daemon.client.call('add_account', { name: 'Claude fixture', provider: 'claude' });
});
test.afterAll(async () => daemon?.close());

const card = (page: Page) => page.getByRole('log').getByRole('region', { name: 'Orchestra' }).last();

async function open(page: Page) {
  const chat = (await daemon.client.call('create_session', { workspace: daemon.workspace })) as { id: string };
  await daemon.client.call('set_session_access_mode', { id: chat.id, mode: 'file-edits' });
  const all = (await daemon.client.call('list_sessions', { search: '', offset: 0 })) as unknown[];
  await page.goto(daemon.url());
  // The newest conversation is first once the list has loaded all of them.
  await expect(page.getByRole('navigation').getByRole('listitem')).toHaveCount(all.length);
  await page.getByRole('navigation').getByRole('button').first().click();
  await expect(page.getByRole('textbox', { name: 'Describe what you want to do' })).toBeVisible();
  return chat;
}
async function send(page: Page, text: string) {
  const box = page.getByRole('textbox', { name: 'Describe what you want to do' });
  await box.fill(text);
  await box.press('Enter');
}
async function toPlan(page: Page) {
  await send(page, TASK);
  await expect(card(page)).toContainText('Which file names?', { timeout: 30_000 });
  await card(page).getByRole('button', { name: 'Use the suggestions' }).click();
  await expect(card(page)).toContainText('Two fixture files', { timeout: 30_000 });
}

test('Orchestra is one switch, and a folder without Git gets an offer to set it up', async ({ page }) => {
  test.setTimeout(90_000);
  await page.goto(daemon.url());
  await page.getByRole('button', { name: 'Settings' }).click();
  await page.getByRole('radio', { name: 'General' }).click({ force: true });
  const toggle = page.getByRole('switch', { name: /Orchestra/ });
  await expect(toggle).not.toBeChecked();
  // The switch follows the daemon: it turns on once the setting is saved.
  await toggle.click({ force: true });
  await expect(toggle).toBeChecked();
  expect(((await daemon.client.call('get_settings')) as { orchestra: boolean }).orchestra).toBe(true);

  await open(page);
  await send(page, TASK);
  const offer = page.getByRole('log').getByRole('region', { name: 'Orchestra' });
  await expect(offer).toContainText('needs Git', { timeout: 30_000 });
  await offer.getByRole('button', { name: 'Set up Git here' }).click();
  await expect(offer).toContainText('Git is ready');
  expect(existsSync(path.join(daemon.workspace, '.git'))).toBe(true);
});

test('a run goes from questions to an edited plan, live lanes and the team result', async ({ page }) => {
  test.setTimeout(150_000);
  await open(page);
  await toPlan(page);
  const plan = card(page);
  await expect(plan).toContainText('2 parts');
  // Leaving a part out and putting it back is local until approval; the last one cannot go.
  await plan.getByRole('listitem', { name: 'First file' }).getByRole('button', { name: 'Leave out' }).click();
  await expect(plan).toContainText('1 part');
  await expect(plan.getByRole('listitem', { name: 'Second file' }).getByRole('button', { name: 'Leave out' })).toBeDisabled();
  await plan.getByRole('listitem', { name: 'First file' }).getByRole('button', { name: 'Put back' }).click();
  await expect(plan).toContainText('2 parts');
  const { violations } = await new AxeBuilder({ page }).include('main').withTags(['wcag2a', 'wcag2aa', 'wcag21aa', 'wcag22aa']).analyze();
  expect(violations.filter((v) => v.impact === 'serious' || v.impact === 'critical').map((v) => v.id)).toEqual([]);
  await plan.getByRole('button', { name: 'Approve and start' }).click();

  const run = card(page);
  await expect(run.getByRole('listitem', { name: 'First file' })).toContainText('ORCHESTRA_WORKER_PARTIAL', { timeout: 30_000 });
  await expect(run.getByRole('listitem', { name: 'First file' })).toContainText('Working');
  await expect(run).toContainText('Finished', { timeout: 90_000 });
  await expect(run).toContainText('2 of 2 done');
  await expect(page.getByRole('log').getByRole('article').last()).toContainText('Team result by Lumen');
  for (const f of ['orchestra-s1.txt', 'orchestra-s2.txt']) expect(existsSync(path.join(daemon.workspace, f)), f).toBe(true);
});

test('a plan can be cancelled before anything runs', async ({ page }) => {
  test.setTimeout(90_000);
  await open(page);
  await toPlan(page);
  await card(page).getByRole('button', { name: 'Cancel' }).click();
  await expect(card(page)).toContainText('Cancelled');
  await expect(card(page).getByRole('button', { name: 'Approve and start' })).toHaveCount(0);
});

test('a run interrupted by a restart resumes and keeps finished work', async ({ page }) => {
  test.setTimeout(180_000);
  await open(page);
  await toPlan(page);
  await card(page).getByRole('button', { name: 'Approve and start' }).click();
  await expect(card(page)).toContainText('ORCHESTRA_WORKER_PARTIAL', { timeout: 30_000 });
  // A crash, not a clean stop: a clean stop ends the run like Stop does, a crash leaves it resumable.
  const exited = new Promise((resolve) => daemon.daemon.once('exit', resolve));
  daemon.daemon.kill();
  await exited;
  // The fixture worker sleeps 8 s; once it has exited, the daemon can prove it stopped.
  await page.waitForTimeout(9_000);
  await daemon.restart();
  await expect(card(page)).toContainText('Interrupted', { timeout: 30_000 });
  await card(page).getByRole('button', { name: 'Resume' }).click();
  await expect(card(page)).toContainText('Finished', { timeout: 120_000 });
  await expect(page.getByRole('log').getByRole('region', { name: 'Orchestra' }).nth(-2)).toContainText('Resumed below');
});
