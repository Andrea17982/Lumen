// Windows debug-only test. PATH points to fixtures, app data to a fresh temp directory.
import { chromium } from 'playwright';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, writeFile, realpath } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';

const root = process.cwd();
const testData = await mkdtemp(path.join(tmpdir(), 'lumen-native-test-'));
const workspace = path.join(testData, 'workspace');
await mkdir(workspace);
const child = spawn(path.join(root, 'src-tauri/target/debug/lumen.exe'), [], {
  cwd: root,
  windowsHide: true,
  stdio: 'pipe',
  env: {
    ...process.env,
    LUMEN_TEST_DATA_DIR: testData,
    PATH: `${path.join(root, 'tests/fixtures/cli')};${process.env.PATH}`,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-port=9224',
  },
});
let errors = '';
child.stderr.on('data', (chunk) => {
  errors += chunk.toString();
});
let browser;
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
try {
  for (let i = 0; i < 100; i++) {
    try {
      browser = await chromium.connectOverCDP('http://127.0.0.1:9224');
      break;
    } catch {
      await sleep(200);
    }
  }
  assert(browser, `WebView2 did not expose its test endpoint: ${errors}`);
  const context = browser.contexts()[0];
  let page;
  for (let i = 0; i < 100; i++) {
    page = context.pages().find((p) => !p.url().startsWith('devtools:'));
    if (page && (await page.locator('h1').count())) break;
    await sleep(100);
  }
  assert(page, 'No Lumen webview');
  await page.getByRole('heading', { name: 'A clear space for a brighter idea.' }).waitFor();
  const invoke = (command, args = {}) =>
    page.evaluate(async ({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), {
      command,
      args,
    });
  const diagnostics = await invoke('diagnostics');
  assert.equal(diagnostics.dataDir, testData);
  assert(diagnostics.claude && diagnostics.gpt);
  const gpt = await invoke('add_account', { name: 'Offline GPT fixture', provider: 'gpt' });
  const claude = await invoke('add_account', {
    name: 'Offline Claude fixture',
    provider: 'claude',
  });
  assert.equal(await invoke('account_status', { id: gpt.id }), true);
  assert.equal(await invoke('account_status', { id: claude.id }), true);
  const catalog = await invoke('list_models', { id: gpt.id, refresh: true });
  assert(catalog.find((m) => m.id === 'gpt-fixture').efforts.includes('ultra'));
  const claudeCatalog = await invoke('list_models', { id: claude.id, refresh: true });
  assert(claudeCatalog.find((m) => m.id === 'opus').efforts.includes('max'));
  await invoke('rename_account', { id: gpt.id, name: 'Renamed GPT fixture' });
  assert(
    (await invoke('list_accounts')).some(
      (a) => a.id === gpt.id && a.name === 'Renamed GPT fixture',
    ),
  );
  const options = {
    mode: 'gpt',
    accountId: gpt.id,
    model: null,
    effort: 'low',
    accessMode: 'read-only',
  };
  async function waitSession(id, status) {
    for (let i = 0; i < 200; i++) {
      const rows = await invoke('list_sessions', { search: '', offset: 0 });
      const s = rows.find((s) => s.id === id);
      if (s?.status === status) return s;
      if (s?.status === 'error')
        throw new Error(JSON.stringify(await invoke('get_messages', { id })));
      await sleep(100);
    }
    throw new Error(`Session did not reach ${status}`);
  }
  const s = await invoke('create_session', { workspace });
  const named = await invoke('add_workspace', {
    name: 'Native fixture workspace',
    folder: workspace,
  });
  const subfolder = path.join(workspace, 'subfolder');
  await mkdir(subfolder);
  const linked = await invoke('set_session_workspace', {
    id: s.id,
    workspaceId: named.id,
    folder: subfolder,
  });
  assert.equal(linked.workspaceId, named.id);
  assert(linked.workspace.endsWith('subfolder'));
  await assert.rejects(() =>
    invoke('send_message', {
      id: s.id,
      text: 'Invalid effort fixture',
      options: { ...options, model: 'gpt-fast-fixture', effort: 'ultra' },
    }),
  );
  await invoke('send_message', { id: s.id, text: 'UI fixture: review this sample', options });
  await waitSession(s.id, 'completed');
  const messages = await invoke('get_messages', { id: s.id });
  assert.equal(messages.length, 2);
  assert(messages[1].text.includes('offline UI fixture'));
  assert(messages[1].activity.some((a) => a.kind === 'file'));
  const cwdEvent = messages[1].activity.find(
    (a) => a.kind === 'tool' && a.text.startsWith('fixture-only: working directory\n'),
  );
  assert(cwdEvent, 'The fixture must report its working directory');
  const actualFolder = cwdEvent.text.split('\n').slice(1).join('\n');
  assert.equal(
    (await realpath(actualFolder)).toLowerCase(),
    (await realpath(subfolder)).toLowerCase(),
    'The child must actually run inside the conversation folder (including Windows short/long paths)',
  );
  await invoke('update_session', { id: s.id, title: 'Offline UI fixture', pinned: true });
  const claudeSession = await invoke('create_session', { workspace });
  await invoke('send_message', {
    id: claudeSession.id,
    text: 'Claude stream fixture',
    options: { ...options, mode: 'claude', accountId: claude.id },
  });
  await waitSession(claudeSession.id, 'completed');
  const streamed = await invoke('get_messages', { id: claudeSession.id });
  assert.equal(streamed[1].text, messages[1].text);
  for (const [text, strategy] of [
    ['Refactor security architecture', 'review'],
    ['Compare alternatives', 'parallel'],
  ]) {
    const run = await invoke('create_session', { workspace });
    const plan = await invoke('send_message', {
      id: run.id,
      text,
      options: { ...options, mode: 'auto', accountId: null },
    });
    assert.equal(plan.strategy, strategy);
    await waitSession(run.id, 'completed');
    const transcript = await invoke('get_messages', { id: run.id });
    assert(
      transcript[1].activity.some(
        (a) => a.kind === (strategy === 'review' ? 'review' : 'perspectives'),
      ),
    );
  }
  const queued = [];
  for (let i = 0; i < 5; i++) {
    const run = await invoke('create_session', { workspace });
    queued.push(run.id);
    await invoke('send_message', { id: run.id, text: 'SLOW_FIXTURE', options });
  }
  const states = await invoke('list_sessions', { search: '', offset: 0 });
  assert(states.some((s) => queued.includes(s.id) && s.status === 'waiting'));
  for (const id of queued) await invoke('cancel_run', { id });
  for (const id of queued) await waitSession(id, 'interrupted');
  await page.reload();
  await page.getByRole('button', { name: /Offline UI fixture/ }).click();
  await page.getByText('This is an offline UI fixture', { exact: false }).waitFor();
  await mkdir(path.join(root, '.impeccable/review'), { recursive: true });
  await page.screenshot({ path: '.impeccable/review/native-chat-fixture.png' });
  await page.locator('.activity > summary').click();
  await page.screenshot({ path: '.impeccable/review/native-activity-fixture.png' });
  await page.getByRole('button', { name: 'GPT', exact: true }).click();
  await page.getByRole('combobox', { name: 'Model', exact: true }).selectOption('gpt-fast-fixture');
  assert.deepEqual(
    await page
      .getByRole('combobox', { name: 'Reasoning effort' })
      .locator('option')
      .evaluateAll((options) => options.map((o) => o.value)),
    ['', 'low', 'medium'],
  );
  await page.getByRole('combobox', { name: 'Permissions' }).selectOption('autopilot');
  await page.getByRole('button', { name: 'Rename selected account' }).click();
  await page.getByRole('textbox', { name: 'Account name' }).fill('Composer renamed account');
  await page.getByRole('button', { name: 'Save account name' }).click();
  await page
    .getByRole('combobox', { name: 'Account', exact: true })
    .getByRole('option', { name: 'Composer renamed account' })
    .waitFor({ state: 'attached' });
  await page.screenshot({ path: '.impeccable/review/native-provider-controls.png' });
  await page.getByRole('button', { name: 'Manage accounts' }).click();
  await page.getByRole('button', { name: 'Rename Composer renamed account' }).click();
  await page.getByRole('textbox', { name: 'Account name' }).fill('Settings renamed account');
  await page.getByRole('button', { name: 'Save account name' }).click();
  await page.getByRole('button', { name: 'Remove Settings renamed account' }).waitFor();
  await page.getByRole('button', { name: 'Close settings' }).click();
  await page.getByRole('dialog').waitFor({ state: 'detached' });
  assert(
    (await invoke('list_accounts')).some(
      (a) => a.id === gpt.id && a.name === 'Settings renamed account',
    ),
  );
  const autopilot = await invoke('create_session', { workspace });
  await invoke('send_message', {
    id: autopilot.id,
    text: 'Offline autopilot flags fixture',
    options: { ...options, accessMode: 'autopilot' },
  });
  await waitSession(autopilot.id, 'completed');
  await writeFile(
    '.impeccable/review/native-results.json',
    JSON.stringify(
      {
        passed: true,
        tests: [
          'isolated app data',
          'account metadata and official status command adapter',
          'session persistence',
          'Codex JSONL',
          'Claude partial streaming',
          'writer-reviewer',
          'parallel synthesis',
          'queue and cancellation',
          'native chat rendering',
        ],
        note: 'Offline CLI fixtures only; no provider accounts or inference were used.',
      },
      null,
      2,
    ),
  );
  console.log('Native offline checks passed. Temporary test data: ' + testData);
  await page.evaluate(() => window.close());
} finally {
  if (browser) await browser.close().catch(() => {});
  if (!child.killed) child.kill();
}
