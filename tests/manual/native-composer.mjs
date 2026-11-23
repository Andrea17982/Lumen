// Focused Windows/WebView2 acceptance. Real IPC/storage, offline CLI fixtures, isolated data.
// The UI transport is held/rejected deliberately to exercise delivery races. No live inference.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
import { createServer } from 'node:net';

const root = process.cwd();
const data = await mkdtemp(path.join(tmpdir(), 'lumen-composer-'));
const workspace = path.join(data, 'workspace');
await mkdir(workspace);
const portServer = createServer();
await new Promise((resolve) => portServer.listen(0, '127.0.0.1', resolve));
const port = portServer.address().port;
await new Promise((resolve) => portServer.close(resolve));
let vite;
try {
  await fetch('http://127.0.0.1:1420');
} catch {
  vite = spawn(process.execPath, ['node_modules/vite/bin/vite.js', '--host', '127.0.0.1'], {
    cwd: root,
    windowsHide: true,
    stdio: 'pipe',
  });
  for (let i = 0; i < 100; i++) {
    try {
      await fetch('http://127.0.0.1:1420');
      break;
    } catch {
      await new Promise((r) => setTimeout(r, 100));
    }
  }
}
const app = spawn(path.join(root, 'src-tauri/target/debug/lumen.exe'), [], {
  cwd: root,
  windowsHide: true,
  stdio: 'pipe',
  env: {
    ...process.env,
    LUMEN_TEST_DATA_DIR: data,
    WEBVIEW2_USER_DATA_FOLDER: path.join(data, 'webview'),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
    PATH: `${path.join(root, 'tests/fixtures/cli')};${process.env.PATH}`,
  },
});
let stderr = '';
app.stderr.on('data', (chunk) => {
  stderr += chunk;
});
let browser;
const checks = [];
const captureDir = path.join(root, '.impeccable/review');
await mkdir(captureDir, { recursive: true });
try {
  for (let i = 0; i < 120; i++) {
    try {
      browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
      break;
    } catch {
      await new Promise((r) => setTimeout(r, 150));
    }
  }
  assert(browser, `No isolated WebView2: ${stderr}`);
  const page = browser
    .contexts()[0]
    .pages()
    .find((p) => !p.url().startsWith('devtools:'));
  assert(page);
  await page.locator('h1').waitFor();
  const invoke = (command, args = {}) =>
    page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), {
      command,
      args,
    });
  assert.equal((await invoke('diagnostics')).dataDir, data);
  const account = await invoke('add_account', { name: 'Offline Claude', provider: 'claude' });
  const first = await invoke('create_session', { workspace });
  const second = await invoke('create_session', { workspace });
  await invoke('update_session', { id: first.id, title: 'First composer chat', pinned: false });
  await invoke('update_session', { id: second.id, title: 'Second composer chat', pinned: false });
  // Test-only transport seam in the dev module, confined to this isolated WebView2 page.
  // Tauri's invoke property is intentionally immutable. Production source remains untouched.
  await page.route('**/src/lib/bridge.ts*', async (route) => {
    const response = await route.fetch();
    const source = await response.text();
    const original = 'if (desktop) return invoke(command, args);';
    assert(source.includes(original), 'The controlled transport seam must match the dev module');
    await route.fulfill({
      response,
      body: source.replace(
        original,
        'if (desktop) return window.__composerIntercept ? window.__composerIntercept(command, args, () => invoke(command, args)) : invoke(command, args);',
      ),
    });
  });
  await page.reload();
  const row = (title) => page.locator('.session-row').filter({ hasText: title });
  async function selectChat(title) {
    const logo = page.locator('.sidebar-toggle');
    if ((await logo.getAttribute('aria-expanded')) === 'false') await logo.click();
    await row(title).click();
  }
  const input = page.getByRole('textbox', { name: 'Message', exact: true });
  await selectChat('First composer chat');
  await input.fill('First draft');
  await page.evaluate(async () => {
    const canvas = document.createElement('canvas');
    canvas.width = canvas.height = 2;
    const blob = await new Promise((resolve) => canvas.toBlob(resolve, 'image/png'));
    const clipboardData = new DataTransfer();
    clipboardData.items.add(new File([blob], 'composer-fixture.png', { type: 'image/png' }));
    document
      .querySelector('textarea')
      .dispatchEvent(new ClipboardEvent('paste', { clipboardData, bubbles: true }));
  });
  await expect(page.getByLabel('Attached files')).toContainText('clipboard-');
  const attachment = await page.locator('.attachment-chip').getAttribute('title');
  await selectChat('Second composer chat');
  await expect(input).toHaveValue('');
  await expect(page.getByLabel('Attached files')).toHaveCount(0);
  await input.fill('Second draft');
  await selectChat('First composer chat');
  await expect(input).toHaveValue('First draft');
  await expect(page.getByLabel('Attached files')).toContainText('clipboard-');
  await page.getByRole('button', { name: 'New conversation', exact: true }).click();
  await expect(input).toHaveValue('');
  await expect(page.getByLabel('Attached files')).toHaveCount(0);
  await selectChat('First composer chat');
  await expect(input).toHaveValue('First draft');
  checks.push('Text and attachments stay with their chat, including New conversation');

  async function holdSend(reject = false) {
    await page.evaluate((reject) => {
      window.__composerSendCalls = 0;
      window.__composerIntercept = (command, args, proceed) => {
        if (command !== 'send_message') return proceed();
        window.__composerSendCalls++;
        return new Promise((resolve, fail) => {
          window.__composerRelease = () =>
            reject ? fail('Controlled rejection before acceptance') : proceed().then(resolve, fail);
        });
      };
    }, reject);
  }
  await input.fill('Send with a controlled acceptance delay');
  await holdSend();
  await page.getByRole('button', { name: 'Send message', exact: true }).click();
  await expect(input).toHaveValue('');
  await expect(page.locator('.message.user')).toContainText(
    'Send with a controlled acceptance delay',
  );
  await expect(page.getByRole('status', { name: '' }).filter({ hasText: 'Sending…' })).toHaveCount(
    1,
  );
  await page.screenshot({
    path: path.join(captureDir, 'native-composer-pending.png'),
    animations: 'disabled',
  });
  await page.evaluate(() => {
    document.documentElement.dataset.theme = 'dark';
  });
  await page.screenshot({
    path: path.join(captureDir, 'native-composer-pending-dark.png'),
    animations: 'disabled',
  });
  await page.evaluate(() => {
    document.documentElement.dataset.theme = 'light';
  });
  await input.fill('Next draft in first chat');
  await input.press('Enter'); // Must not duplicate a submission during the acceptance window.
  assert.equal(await page.evaluate(() => window.__composerSendCalls), 1);
  await selectChat('Second composer chat');
  await expect(input).toHaveValue('Second draft');
  await page.evaluate(() => window.__composerRelease());
  await expect.poll(async () => (await invoke('get_messages', { id: first.id })).length).toBe(2);
  await expect
    .poll(
      async () =>
        (await invoke('list_sessions', { search: '', offset: 0 })).find((s) => s.id === first.id)
          ?.status,
    )
    .toBe('completed');
  await expect(input).toHaveValue('Second draft');
  await expect(page.locator('.conversation')).not.toContainText(
    'Send with a controlled acceptance delay',
  );
  await selectChat('First composer chat');
  await expect(input).toHaveValue('Next draft in first chat');
  await expect(page.locator('.message.user')).toHaveCount(1);
  const stored = await invoke('get_messages', { id: first.id });
  assert.equal(stored[0].activity.find((a) => a.kind === 'attachments').text, attachment);
  const options = {
    mode: 'claude',
    accountId: account.id,
    model: 'sonnet',
    effort: '',
    accessMode: 'read-only',
  };
  await assert.rejects(() =>
    invoke('send_message', {
      id: first.id,
      text: 'Must not replay',
      clientMessageId: stored[0].id,
      options,
    }),
  );
  assert.equal((await invoke('get_messages', { id: first.id })).length, 2);
  checks.push(
    'Immediate pending bubble, single send, stable client ID and no late draft/chat overwrite',
  );

  await holdSend(true);
  await input.fill('Rejected original draft');
  await page.getByRole('button', { name: 'Send message', exact: true }).click();
  await expect(input).toHaveValue('');
  await input.fill('Newer text must survive');
  await selectChat('Second composer chat');
  await page.evaluate(() => window.__composerRelease());
  await expect(input).toHaveValue('Second draft');
  await selectChat('First composer chat');
  await expect(input).toHaveValue('Rejected original draft\n\nNewer text must survive');
  await expect(page.getByRole('alert')).toContainText('Draft restored');
  assert.equal((await invoke('get_messages', { id: first.id })).length, 2);
  checks.push('Rejection restores the source draft and preserves newer text; no replay');

  await page.getByRole('button', { name: 'Dismiss error', exact: true }).click();
  // The backend accepts the message, but the first readback fails. Retrying the read must not
  // invoke the provider again, nor restore an accepted prompt as an unsent draft.
  await page.evaluate(() => {
    window.__composerSendCalls = 0;
    window.__composerBlockRead = false;
    window.__composerIntercept = async (command, args, proceed) => {
      if (command === 'send_message') {
        window.__composerSendCalls++;
        const result = await proceed();
        window.__composerBlockRead = true;
        return result;
      }
      if (command === 'get_messages' && window.__composerBlockRead)
        throw new Error('Controlled readback failure');
      return proceed();
    };
  });
  await input.fill('Accepted with unavailable readback');
  await page.getByRole('button', { name: 'Send message', exact: true }).click();
  await expect(input).toHaveValue('');
  const checkDelivery = page.getByRole('button', { name: 'Check message delivery', exact: true });
  await expect(checkDelivery).toBeVisible();
  await page.screenshot({
    path: path.join(captureDir, 'native-composer-uncertain.png'),
    animations: 'disabled',
  });
  await expect
    .poll(
      async () =>
        (await invoke('list_sessions', { search: '', offset: 0 })).find((s) => s.id === first.id)
          ?.status,
    )
    .toBe('completed');
  await input.fill('Draft after accepted message');
  await page.evaluate(() => {
    window.__composerBlockRead = false;
  });
  await checkDelivery.click();
  await expect(checkDelivery).toHaveCount(0);
  await expect(input).toHaveValue('Draft after accepted message');
  assert.equal(await page.evaluate(() => window.__composerSendCalls), 1);
  assert.equal(
    (await invoke('get_messages', { id: first.id })).filter(
      (m) => m.text === 'Accepted with unavailable readback',
    ).length,
    1,
  );
  checks.push('Uncertain readback is checked without resend, duplicate or draft overwrite');

  await page.evaluate(() => {
    window.__composerBlockRead = false;
    window.__composerSendCalls = 0;
    window.__composerIntercept = async (command, args, proceed) => {
      if (command === 'send_message') {
        window.__composerSendCalls++;
        window.__composerBlockRead = true;
        throw 'Controlled rejection with unavailable readback';
      }
      if (command === 'get_messages' && window.__composerBlockRead)
        throw new Error('Controlled readback failure');
      return proceed();
    };
  });
  await input.fill('Rejected while readback unavailable');
  await page.getByRole('button', { name: 'Send message', exact: true }).click();
  await expect(checkDelivery).toBeVisible();
  await input.fill('Still preserve newer text');
  await page.evaluate(() => {
    window.__composerBlockRead = false;
  });
  await checkDelivery.click();
  await expect(checkDelivery).toHaveCount(0);
  await expect(input).toHaveValue(
    'Rejected while readback unavailable\n\nStill preserve newer text',
  );
  await expect(page.getByRole('alert')).toContainText('Draft restored');
  assert.equal(await page.evaluate(() => window.__composerSendCalls), 1);
  checks.push(
    'Rejected send with failed first readback remains recoverable after checking delivery',
  );

  await page.evaluate(() => {
    window.__composerIntercept = undefined;
  });
  await page.getByRole('button', { name: 'New conversation', exact: true }).click();
  // The existing project selection supplies the folder; do not open a native dialog.
  await page.evaluate((folder) => localStorage.setItem('lumen-workspace', folder), workspace);
  // New conversation inherits the selected folder in this window, set via its normal command.
  // Use a disposable chat to cover creation through the UI with an explicit known workspace.
  await page.reload();
  await input.fill('First message creates a new conversation');
  await page.getByRole('button', { name: 'Send message', exact: true }).click();
  await expect(input).toHaveValue('');
  await expect(page.locator('.message.user')).toContainText(
    'First message creates a new conversation',
  );
  await input.fill('Follow-up draft in newly created chat');
  await expect
    .poll(
      async () =>
        (await invoke('list_sessions', { search: '', offset: 0 })).find((s) =>
          s.title.startsWith('First message creates'),
        )?.status,
    )
    .toBe('completed');
  await expect(input).toHaveValue('Follow-up draft in newly created chat');
  await selectChat('Second composer chat');
  await expect(input).toHaveValue(''); // Window-local drafts intentionally do not survive reload.
  await row('First message creates a new conversation')
    .filter({ hasNotText: '· continued' })
    .click();
  await expect(input).toHaveValue('Follow-up draft in newly created chat');
  checks.push('New-chat creation keeps its optimistic message and follow-up draft isolated');

  await page.evaluate(() => {
    window.__composerIntercept = (command, args, proceed) => {
      if (command !== 'compact_session') return proceed();
      return new Promise((resolve, reject) => {
        window.__composerCompactRelease = () => proceed().then(resolve, reject);
      });
    };
  });
  await input.fill('/compact');
  await input.press('Escape'); // Dismiss command suggestions; Enter executes the command.
  await input.press('Enter');
  await expect
    .poll(() => page.evaluate(() => typeof window.__composerCompactRelease))
    .toBe('function');
  await input.fill('New text while compacting the original chat');
  await selectChat('Second composer chat');
  await input.fill('Do not leave this chat');
  await page.evaluate(() => window.__composerCompactRelease());
  await expect(input).toHaveValue('Do not leave this chat');
  await expect(page.locator('.conversation-label')).toContainText('Second composer chat');
  await row('First message creates a new conversation')
    .filter({ hasNotText: '· continued' })
    .click();
  await expect(input).toHaveValue('New text while compacting the original chat');
  checks.push(
    'Delayed Compact preserves newer source text and does not navigate away from another chat',
  );

  await mkdir(path.join(root, '.impeccable/review'), { recursive: true });
  await page.screenshot({ path: path.join(root, '.impeccable/review/native-composer.png') });
  await writeFile(
    path.join(root, '.impeccable/review/native-composer-results.json'),
    JSON.stringify(
      {
        at: new Date().toISOString(),
        passed: true,
        checks,
        scope:
          'Windows debug WebView2, real IPC/SQLite and offline Claude CLI fixture; controlled UI transport delay/rejection. No live inference or release performance claim.',
      },
      null,
      2,
    ),
  );
  process.stdout.write(JSON.stringify({ passed: true, checks, data }, null, 2) + '\n');
  await invoke('quit_app').catch((error) => {
    // The owned app can close its webview before the Quit IPC acknowledgment arrives.
    if (!String(error).includes('Target page, context or browser has been closed')) throw error;
  });
} finally {
  await browser?.close().catch(() => {});
  if (app.exitCode === null) app.kill();
  if (vite?.exitCode === null) vite.kill();
}
