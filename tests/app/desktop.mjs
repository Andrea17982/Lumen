// The real desktop app, driven over the embedded WebDriver server of a test build
// (cargo feature `webdriver`). It checks what only the shell does: start the bundled service,
// relay its events, and recover on its own when the service dies mid-turn. Offline CLI fixtures;
// opens a Lumen window on the desktop for about half a minute.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { daemonClient, sleep } from '../daemon-harness.mjs';
import { alive, daemonPath, launchApp, requireBuilds } from '../support/app.mjs';

if (process.platform !== 'win32') {
  console.log(JSON.stringify({ skipped: 'CLI fixtures are Windows-only' }));
  process.exit(0);
}
requireBuilds();

const data = await mkdtemp(path.join(tmpdir(), 'lumen-app-'));
const workspace = path.join(data, 'workspace');
await mkdir(workspace);
const client = await daemonClient(data, daemonPath);
const checks = [];
let app;
try {
  app = await launchApp(data);
  const { browser } = app;
  const status = "return document.querySelector('[role=status]')?.textContent";
  await browser.until(status, (s) => s === 'Connected' || s === 'Connesso', 'connection');
  const first = await client.call('__status');
  checks.push('the shell started the bundled service and connected to it');

  const account = await client.call('add_account', { name: 'App fixture', provider: 'claude' });
  await client.call('add_workspace', { name: 'Fixture', folder: workspace });
  const chat = await client.call('create_session', { workspace });
  await client.call('send_message', {
    id: chat.id,
    text: 'DAEMON_TURN_FIXTURE',
    options: { mode: 'claude', accountId: account.id, model: 'sonnet', effort: 'low', accessMode: 'autopilot', autoReview: false },
  });
  // create_session emits no event: the session appears when its turn starts.
  await browser.until("return document.querySelectorAll('nav li').length", (n) => n === 1, 'session in the sidebar');
  await browser.click('nav li button');
  const log = "return document.querySelector('[role=log]')?.innerText ?? ''";
  await browser.until(log, (t) => t.includes('DAEMON_PARTIAL'), 'streamed partial reply');
  checks.push('daemon events reach the window through the shell relay');

  execFileSync('taskkill', ['/PID', String(first.pid), '/F'], { stdio: 'ignore', windowsHide: true });
  await browser.until(status, (s) => s !== 'Connected' && s !== 'Connesso', 'disconnection', 10_000);
  const header = "return document.querySelector('main header')?.innerText ?? ''";
  await browser.until(header, (h) => /Interrupted|Interrotta/.test(h), 'interrupted turn after restart', 40_000);
  await browser.until(status, (s) => s === 'Connected' || s === 'Connesso', 'reconnection');
  const restarted = await client.call('__status');
  assert.notEqual(restarted.pid, first.pid);
  const text = await browser.run(log);
  assert.equal(text.split('DAEMON_PARTIAL').length - 1, 1, text);
  checks.push('after a service crash the shell restarts it, and the window shows the partial reply once and the turn interrupted');
  console.log(JSON.stringify({ passed: true, checks }, null, 2));
} finally {
  await app?.close();
  // The service outlives the window by design; stop the one this test started.
  const pid = await client
    .call('__status')
    .then((s) => s.pid)
    .catch(() => null);
  await client.call('__shutdown').catch(() => {});
  for (let i = 0; pid && i < 50 && alive(pid); i++) await sleep(100);
  await rm(data, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
}
