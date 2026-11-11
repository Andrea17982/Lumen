// Real Windows debug/WebView2 + IPC/SQLite; provider behavior is an offline CLI fixture.
import { expect } from '@playwright/test';
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { nativeHarness } from './native-harness.mjs';

const h = await nativeHarness('execution');
const { page, invoke, workspace, root } = h;
const captures = path.join(root, '.impeccable/review');
await mkdir(captures, { recursive: true });
const checks = [];
try {
  await invoke('add_account', { name: 'Offline Claude', provider: 'claude' });
  const input = page.getByRole('textbox', { name: 'Message', exact: true });
  const summary = page.getByLabel('Execution progress', { exact: true });
  const header = summary.locator(':scope > summary');
  async function chat(title) {
    const session = await invoke('create_session', { workspace });
    await invoke('update_session', { id: session.id, title, pinned: false });
    await page.reload();
    await page.locator('.session-row').filter({ hasText: title }).click();
    return session;
  }
  async function send(text) {
    await input.fill(text);
    await page.getByRole('button', { name: 'Send message', exact: true }).click();
  }
  async function ended(id, status = 'completed') {
    await expect
      .poll(
        async () =>
          (await invoke('list_sessions', { search: '', offset: 0 })).find((s) => s.id === id)
            ?.status,
        { timeout: 20000 },
      )
      .toBe(status);
  }
  const traces = async (id) =>
    (await invoke('get_messages', { id }))
      .flatMap((m) => m.activity)
      .filter((a) => a.kind === 'execution')
      .map((a) => JSON.parse(a.text));

  const q = await chat('Question execution');
  await send('QUESTION_FIXTURE');
  await expect(header).toContainText('Waiting for your answer', { timeout: 20000 });
  await header.click();
  await expect(summary).not.toContainText('This does not prove a stall');
  await page.screenshot({
    path: path.join(captures, 'execution-question.png'),
    animations: 'disabled',
  });
  await page
    .locator('.native-questions')
    .getByRole('radio', { name: /Native/ })
    .check();
  await page
    .locator('.native-questions')
    .getByRole('button', { name: 'Send answers', exact: true })
    .click();
  await ended(q.id);
  await expect(header).toContainText('Turn ended');
  assert((await traces(q.id)).every((s) => s.pendingQuestions === 0 && s.endedAt));
  checks.push('Pending native question is distinct from silence and clears after the real answer');

  const promise = await chat('Promise execution');
  await send('OBS_PROMISE_FIXTURE');
  await ended(promise.id);
  await expect(header).toContainText('Turn ended');
  const promiseTrace = (await traces(promise.id)).at(-1);
  assert.equal(promiseTrace.phase, 'returned');
  assert.equal(promiseTrace.toolEvents, 0);
  await header.click();
  await expect(summary).toContainText('does not certify');
  await page.reload();
  await page.locator('.session-row').filter({ hasText: 'Promise execution' }).click();
  await expect(header).toContainText('Turn ended');
  assert.equal((await traces(promise.id)).at(-1).toolEvents, 0);
  checks.push('Promise-only response ends with zero tool evidence; diagnostics survive reload');

  const tool = await chat('Tool execution');
  await send('OBS_TOOL_FIXTURE');
  await expect(header).toContainText('Tool operation open', { timeout: 20000 });
  await ended(tool.id);
  await expect(header).toContainText('Turn ended');
  const toolTrace = (await traces(tool.id)).at(-1);
  assert.equal(toolTrace.pendingTools, 0);
  assert(toolTrace.timeline.some(([phase]) => phase === 'tool'));
  checks.push(
    'Explicit tool start/end is observed without treating a completed result as still running',
  );

  const failure = await chat('Error execution');
  await send('OBS_ERROR_FIXTURE');
  await ended(failure.id, 'error');
  await expect(header).toContainText('Turn failed');
  assert.equal((await traces(failure.id)).at(-1).phase, 'error');
  checks.push('Provider failure is separate from normal return and no request is replayed');

  const silent = await chat('Silent execution');
  await send('SLOW_FIXTURE');
  await expect(header).toContainText('Waiting for provider', { timeout: 20000 });
  await header.click();
  await expect(summary).toContainText('This does not prove a stall', { timeout: 40000 });
  const before = (await invoke('get_messages', { id: silent.id })).filter(
    (m) => m.role === 'user',
  ).length;
  await page.screenshot({
    path: path.join(captures, 'execution-silence.png'),
    animations: 'disabled',
  });
  await summary.getByRole('button', { name: 'Refresh execution status', exact: true }).click();
  assert.equal(
    (await invoke('get_messages', { id: silent.id })).filter((m) => m.role === 'user').length,
    before,
  );
  await expect(header).toContainText('Waiting for provider');
  await page.getByRole('button', { name: 'Stop response', exact: true }).click();
  await ended(silent.id, 'interrupted');
  await expect(header).toContainText('Turn interrupted');
  const stopped = (await traces(silent.id)).at(-1);
  assert.equal(stopped.phase, 'cancelled');
  checks.push(
    'Actual 30-second silence is visible, status refresh is read-only, explicit Stop cancels without restart',
  );

  // Hold a real read-only snapshot while the user moves to another chat.
  await page.route('**/src/lib/bridge.ts*', async (route) => {
    const response = await route.fetch();
    const source = await response.text();
    const original = 'if (desktop) return invoke(command, args);';
    assert(source.includes(original));
    await route.fulfill({
      response,
      body: source.replace(
        original,
        'if (desktop) return window.__executionIntercept ? window.__executionIntercept(command, args, () => invoke(command, args)) : invoke(command, args);',
      ),
    });
  });
  await page.reload();
  await page.locator('.session-row').filter({ hasText: 'Silent execution' }).click();
  await header.click();
  await page.evaluate((id) => {
    window.__executionIntercept = async (command, args, proceed) => {
      const value = await proceed();
      if (command === 'get_messages' && args.id === id)
        return new Promise((resolve) => {
          window.__releaseExecutionRead = () => resolve(value);
        });
      return value;
    };
  }, silent.id);
  await summary.getByRole('button', { name: 'Refresh execution status', exact: true }).click();
  await expect
    .poll(() => page.evaluate(() => typeof window.__releaseExecutionRead))
    .toBe('function');
  await page.locator('.session-row').filter({ hasText: 'Promise execution' }).click();
  await input.fill('Keep this new draft in the selected chat');
  await page.evaluate(() => window.__releaseExecutionRead());
  await expect(input).toHaveValue('Keep this new draft in the selected chat');
  await expect(page.locator('.conversation-label')).toContainText('Promise execution');
  await expect(page.locator('.conversation')).toContainText('I will implement');
  checks.push('Delayed read-only status refresh cannot overwrite another chat or its draft');

  const prefs = await invoke('get_settings');
  await invoke('save_settings', { value: { ...prefs, orchestra: true } });
  const orchestra = await chat('Orchestra observation');
  const plan = await invoke('send_message', {
    id: orchestra.id,
    text: 'Implement the authentication architecture for the API and the settings UI',
    options: { mode: 'auto', accountId: null, model: null, effort: '', accessMode: 'read-only' },
  });
  assert.equal(plan.strategy, 'orchestra');
  await ended(orchestra.id);
  await expect(header).toContainText('Waiting for your answer');
  assert((await traces(orchestra.id)).length > 0);
  await page.reload();
  await page.locator('.session-row').filter({ hasText: 'Orchestra observation' }).click();
  await expect(header).toContainText('Waiting for your answer');
  assert((await traces(orchestra.id)).every((s) => s.endedAt));
  checks.push('Internal Orchestra interview keeps provider observations after save/reload');
  await invoke('save_settings', { value: prefs });

  await page.evaluate(() => {
    localStorage.setItem('lumen-language', 'it');
    localStorage.setItem('lumen-theme', 'dark');
  });
  await page.reload();
  await page.locator('.session-row').filter({ hasText: 'Silent execution' }).click();
  const italian = page.getByLabel('Avanzamento dell’esecuzione', { exact: true });
  await expect(italian.locator(':scope > summary')).toContainText('Turno interrotto');
  await italian.locator(':scope > summary').click();
  await page.screenshot({
    path: path.join(captures, 'execution-dark-it.png'),
    animations: 'disabled',
  });
  checks.push('Persisted interruption is readable after reload in Italian/dark theme');
  await writeFile(
    path.join(captures, 'native-execution-results.json'),
    JSON.stringify(
      {
        at: new Date().toISOString(),
        passed: true,
        checks,
        scope:
          'Windows debug WebView2, real IPC/SQLite and offline CLI fixture. Real silence duration; no live inference or release-performance claim.',
      },
      null,
      2,
    ),
  );
  process.stdout.write(JSON.stringify({ passed: true, checks }, null, 2) + '\n');
  await invoke('quit_app').catch((error) => {
    // Quit may close the owned webview before IPC acknowledges the command.
    if (!String(error).includes('Target page, context or browser has been closed')) throw error;
  });
} finally {
  await h.close();
}
