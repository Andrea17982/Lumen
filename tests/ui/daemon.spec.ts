// M2 exit check against the real daemon and the offline Claude CLI fixture, through the test
// bridge: killing the service mid-stream and restarting it shows the same partial text once,
// with the turn marked interrupted.
import { expect, test } from '@playwright/test';
import { sleep } from '../daemon-harness.mjs';
import { startDaemon } from './daemon-fixture';

// The CLI fixtures are Windows command scripts.
test.skip(process.platform !== 'win32', 'CLI fixtures are Windows-only');

test('a daemon crash mid-stream keeps the partial reply once and shows the turn interrupted', async ({ page, baseURL }) => {
  test.setTimeout(120_000);
  const daemon = await startDaemon(new URL(baseURL!).origin);
  try {
    const { client } = daemon;
    const account = await client.call('add_account', { name: 'UI fixture', provider: 'claude' });
    const chat = await client.call('create_session', { workspace: daemon.workspace });

    await page.goto(daemon.url());
    await expect(page.getByRole('status')).toHaveText('Connected');
    await page.getByRole('navigation').getByRole('button').first().click();

    await client.call('send_message', {
      id: chat.id,
      text: 'DAEMON_TURN_FIXTURE',
      options: { mode: 'claude', accountId: account.id, model: 'sonnet', effort: 'low', accessMode: 'autopilot', autoReview: false },
    });
    const log = page.getByRole('log');
    await expect(log).toContainText('DAEMON_PARTIAL', { timeout: 30_000 });
    await expect(page.locator('main header')).toContainText('Running');

    daemon.daemon.kill('SIGKILL');
    await expect(page.getByRole('status')).toHaveText('Reconnecting…');
    await expect(page.getByText('What you see may be out of date')).toBeVisible();
    await sleep(500);
    await daemon.restart();

    await expect(page.getByRole('status')).toHaveText('Connected', { timeout: 15_000 });
    await expect(page.locator('main header')).toContainText('Interrupted');
    await expect(page.getByText('What you see may be out of date')).toBeHidden();
    const text = await log.innerText();
    expect(text.split('DAEMON_PARTIAL').length - 1, text).toBe(1);
  } finally {
    await daemon.close();
  }
});
