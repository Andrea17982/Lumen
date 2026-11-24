// M2 exit check against the real daemon and the offline Claude CLI fixture, through the test
// bridge: killing the service mid-stream and restarting it shows the same partial text once,
// with the turn marked interrupted.
import { expect, test } from '@playwright/test';
import { existsSync } from 'node:fs';
import { mkdir, mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { daemonClient, sleep } from '../daemon-harness.mjs';
import { startBridge } from '../support/ipc-bridge.mjs';

// The CLI fixtures are Windows command scripts.
test.skip(process.platform !== 'win32', 'CLI fixtures are Windows-only');

test('a daemon crash mid-stream keeps the partial reply once and shows the turn interrupted', async ({ page, baseURL }) => {
  test.setTimeout(120_000);
  const executable = path.resolve('src-tauri/target/debug/lumen-daemon.exe');
  expect(existsSync(executable), 'build the daemon first: cargo build -p lumen-daemon').toBe(true);
  const data = await mkdtemp(path.join(tmpdir(), 'lumen-ui-'));
  const workspace = path.join(data, 'workspace');
  await mkdir(workspace);
  const client = await daemonClient(data, executable);
  let daemon = client.start();
  let bridge: Awaited<ReturnType<typeof startBridge>> | undefined;
  try {
    await client.ready();
    bridge = await startBridge(client, { origin: new URL(baseURL!).origin });
    const account = await client.call('add_account', { name: 'UI fixture', provider: 'claude' });
    const chat = await client.call('create_session', { workspace });

    await page.goto(`/?locale=en&bridge=${encodeURIComponent(bridge.url)}&token=${bridge.token}`);
    await expect(page.getByRole('status')).toHaveText('Connected');
    await page.getByRole('navigation').getByRole('button').first().click();

    await client.call('send_message', {
      id: chat.id,
      text: 'DAEMON_TURN_FIXTURE',
      options: { mode: 'claude', accountId: account.id, model: 'sonnet', effort: 'low', accessMode: 'autopilot', autoReview: false },
    });
    const log = page.getByRole('log');
    await expect(log).toContainText('DAEMON_PARTIAL', { timeout: 30_000 });
    await expect(page.locator('header')).toContainText('Running');

    daemon.kill('SIGKILL');
    await expect(page.getByRole('status')).toHaveText('Reconnecting…');
    await expect(page.getByText('What you see may be out of date')).toBeVisible();
    await sleep(500);
    daemon = client.start();
    await client.ready();

    await expect(page.getByRole('status')).toHaveText('Connected', { timeout: 15_000 });
    await expect(page.locator('header')).toContainText('Interrupted');
    await expect(page.getByText('What you see may be out of date')).toBeHidden();
    const text = await log.innerText();
    expect(text.split('DAEMON_PARTIAL').length - 1, text).toBe(1);
  } finally {
    await bridge?.close();
    await client.call('__shutdown').catch(() => {});
    if (daemon.exitCode === null) daemon.kill();
    await new Promise((resolve) => (daemon.exitCode !== null || daemon.signalCode !== null ? resolve(null) : daemon.once('exit', resolve)));
    await rm(data, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
  }
});
