// The real desktop app, driven over the embedded WebDriver server of a test build
// (cargo feature `webdriver`). It checks what only the shell does: start the bundled service,
// relay its events, and recover on its own when the service dies mid-turn. Offline CLI fixtures;
// opens a Lumen window on the desktop for about half a minute.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
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
let probe;
try {
  app = await launchApp(data);
  let { browser } = app;
  await client.ready();
  await client.call('onboarding_state', { action: 'skipSetup' });
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
  await app.close();
  app = undefined;
  app = await launchApp(data);
  browser = app.browser;
  await browser.until(status, (s) => s === 'Connected' || s === 'Connesso', 'connection after reopening');
  await browser.until("return document.querySelector('nav button[aria-current=page]')?.textContent", (s) => s?.includes('DAEMON_TURN_FIXTURE'), 'restored conversation in its project');
  await browser.until(log, (t) => t.split('DAEMON_PARTIAL').length - 1 === 1, 'restored partial transcript');
  assert.equal((await client.call('__status')).pid, restarted.pid);
  checks.push('reopening the shell restores the selected conversation and partial output without relaunching the daemon');

  // A plugin panel served by lumen-plugin: tries to reach the shell's IPC. Tauri's own invoke
  // falls back to postMessage when the panel's CSP blocks fetch, so this covers both paths.
  probe = await mkdtemp(path.join(tmpdir(), 'lumen-probe-'));
  await writeFile(path.join(probe, 'lumen-plugin.json'), JSON.stringify({ id: 'ipc-probe', name: 'IPC probe', version: '1.0.0', lumen: '>=0.10.0 <1.0.0', contributes: { panels: [{ id: 'main', title: 'Probe', entry: 'panel.html' }] } }));
  await writeFile(
    path.join(probe, 'panel.html'),
    `<!doctype html><script>
(async () => {
  const out = { internals: typeof window.__TAURI_INTERNALS__, ipc: typeof window.ipc, webview: typeof (window.chrome && window.chrome.webview) };
  const call = window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.invoke;
  if (call) {
    out.invoke = await Promise.race([
      call('daemon_call', { command: 'rename_account', args: { id: ${JSON.stringify(account.id)}, name: 'PANEL_REACHED_IPC' } }).then(() => 'ALLOWED', (e) => 'DENIED ' + e),
      new Promise((resolve) => setTimeout(() => resolve('NO_ANSWER'), 3000)),
    ]);
  }
  parent.postMessage({ probe: out }, '*');
})();
</script>`,
  );
  const inspected = await client.call('plugin_inspect', { source: probe });
  const installed = await client.call('plugin_install', { token: inspected.token });
  await client.call('plugin_set_enabled', { id: installed.id, on: true });
  await browser.run(`
    window.__probe = null;
    addEventListener('message', (e) => { if (e.data && e.data.probe) window.__probe = e.data.probe; });
    const frame = document.createElement('iframe');
    frame.sandbox = 'allow-scripts';
    frame.src = 'http://lumen-plugin.localhost/ipc-probe/main';
    document.body.append(frame);
  `);
  const result = await browser.until('return window.__probe', (v) => !!v, 'panel probe', 15_000);
  await sleep(1000);
  const [renamed] = await client.call('list_accounts');
  assert.notEqual(renamed.name, 'PANEL_REACHED_IPC', JSON.stringify(result));
  assert.notEqual(result.invoke, 'ALLOWED', JSON.stringify(result));
  checks.push(`a sandboxed plugin panel cannot call the shell (${JSON.stringify(result)})`);
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
  if (probe) await rm(probe, { recursive: true, force: true });
}
