import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { nativeHarness } from './native-harness.mjs';
const h = await nativeHarness('081-cursor', {
  appEnv: { CURSOR_API_KEY: 'fixture-key-must-not-reach-cli' },
});
const checks = [];
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function done(id) {
  for (let i = 0; i < 600; i++) {
    const s = (await h.invoke('list_sessions', { search: '', offset: 0 })).find((x) => x.id === id);
    if (['completed', 'error', 'interrupted'].includes(s.status))
      return { s, m: (await h.invoke('get_messages', { id })).at(-1) };
    await sleep(100);
  }
  throw Error('Turn did not finish');
}
const options = (id, accessMode = 'autopilot') => ({
  mode: 'cursor',
  accountId: id,
  model: 'gpt-6.1-sol',
  effort: '',
  accessMode,
  autoReview: false,
});
const attempts = async () =>
  (await readFile(path.join(h.workspace, 'cursor-attempts.jsonl'), 'utf8'))
    .trim()
    .split(/\r?\n/)
    .map((x) => JSON.parse(x));
try {
  await h.invoke('save_settings', {
    value: {
      ...(await h.invoke('get_settings')),
      verifyChanges: false,
      repoContext: false,
      orchestra: false,
      closeToTray: false,
    },
  });
  const a = await h.invoke('add_account', { provider: 'cursor', name: 'Cursor fixture' });
  await assert.rejects(
    h.invoke('add_account', { provider: 'cursor', name: 'Duplicate native profile' }),
  );
  assert.equal((await h.invoke('list_models', { id: a.id }))[1].id, 'gpt-6.1-sol');
  assert.equal(await h.invoke('account_status', { id: a.id }), true);
  assert.equal(
    (await h.invoke('adapter_capabilities')).adapters.find((x) => x.provider === 'cursor').askFirst
      .support,
    'unavailable',
  );
  checks.push(
    'Stored Cursor identity, discovered catalog, capability matrix and single native profile',
  );
  const s = await h.invoke('create_session', { workspace: h.workspace });
  await h.invoke('send_message', {
    id: s.id,
    text: 'CURSOR_FIRST ' + 'Italiano è UTF-8. '.repeat(2400),
    options: options(a.id),
  });
  const first = await done(s.id);
  assert.equal(first.s.status, 'completed');
  assert.equal(first.m.text, 'CURSOR_FIRST_OK');
  assert(first.m.activity.some((x) => x.kind === 'native'));
  assert(first.m.activity.some((x) => x.kind === 'tool'));
  assert((await attempts())[0].length > 32000);
  await h.invoke('send_message', { id: s.id, text: 'Prosegui.', options: options(a.id) });
  assert.equal((await done(s.id)).m.text, 'CURSOR_RESUME_OK');
  const calls = await attempts();
  assert.equal(calls.length, 2);
  assert.equal(calls[1].resumed, true);
  assert.equal(calls[0].session, calls[1].session);
  assert.equal(
    (await readFile(path.join(h.workspace, 'cursor-effects.txt'), 'utf8')).trim().split(/\r?\n/)
      .length,
    1,
  );
  checks.push(
    'Long UTF-8 stdin, no duplicated terminal text, native continuation without repeating file work; inherited API key removed',
  );
  for (const access of ['read-only', 'file-edits', 'ask']) {
    const c = await h.invoke('create_session', { workspace: h.workspace });
    try {
      await h.invoke('send_message', {
        id: c.id,
        text: 'Do not broaden permissions.',
        options: options(a.id, access),
      });
      assert.equal((await done(c.id)).s.status, 'error');
    } catch (e) {
      assert(String(e).includes('Ask') || String(e).includes('approval'), String(e));
    }
  }
  assert.equal((await attempts()).length, 2);
  checks.push('Read only, File edits and Ask first reject before any Cursor execution');
  const truncated = await h.invoke('create_session', { workspace: h.workspace });
  await h.invoke('send_message', {
    id: truncated.id,
    text: 'CURSOR_TRUNCATED',
    options: options(a.id),
  });
  const incomplete = await done(truncated.id);
  assert.equal(incomplete.s.status, 'error');
  assert(incomplete.m.text.includes('Partial retained'));
  assert(incomplete.m.activity.some((x) => x.kind === 'error' && x.text.includes('terminal')));
  checks.push('Truncated stream fails while preserving partial assistant output');
  const quota = await h.invoke('create_session', { workspace: h.workspace });
  await h.invoke('send_message', {
    id: quota.id,
    text: 'CURSOR_QUOTA_FIXTURE',
    options: options(a.id),
  });
  const failed = await done(quota.id);
  assert.equal(failed.s.status, 'error');
  assert(failed.m.text.includes('Existing file work retained'));
  assert(failed.m.activity.some((x) => x.kind === 'quota-reset'));
  const effects = await readFile(path.join(h.workspace, 'cursor-effects.txt'), 'utf8');
  await h.invoke('resume_at_reset', { id: quota.id, message: failed.m.id });
  assert.equal((await done(quota.id)).s.status, 'completed');
  assert.equal(await readFile(path.join(h.workspace, 'cursor-effects.txt'), 'utf8'), effects);
  checks.push(
    'Quota retains native session and partial work; scheduled resume completes without replaying the write',
  );
  const stderr = await h.invoke('create_session', { workspace: h.workspace });
  await h.invoke('send_message', {
    id: stderr.id,
    text: 'CURSOR_STDERR_QUOTA',
    options: options(a.id),
  });
  const rejected = await done(stderr.id);
  assert.equal(rejected.s.status, 'error');
  assert(rejected.m.activity.some((x) => x.kind === 'quota-reset'));
  checks.push('Quota reported only on stderr is classified without exposing raw diagnostics');
  const pinned = await h.invoke('create_session', { workspace: h.workspace });
  const plan = await h.invoke('send_message', {
    id: pinned.id,
    text: 'Use Cursor: continue this task.',
    options: { mode: 'auto', effort: '', accessMode: 'autopilot', autoReview: false },
  });
  assert.equal(plan.provider, 'cursor');
  assert.equal((await done(pinned.id)).s.status, 'completed');
  const automatic = await h.invoke('create_session', { workspace: h.workspace });
  await assert.rejects(
    h.invoke('send_message', {
      id: automatic.id,
      text: 'Explain this project.',
      options: { mode: 'auto', effort: '', accessMode: 'autopilot', autoReview: false },
    }),
  );
  checks.push('Explicit Auto provider instruction selects Cursor; ordinary Auto does not admit it');
  const slow = await h.invoke('create_session', { workspace: h.workspace });
  await h.invoke('send_message', {
    id: slow.id,
    text: 'CURSOR_SLOW_FIXTURE',
    options: options(a.id),
  });
  let pids;
  for (let i = 0; i < 100; i++) {
    try {
      pids = JSON.parse(await readFile(path.join(h.workspace, 'cursor-pids.json'), 'utf8'));
      break;
    } catch {
      await sleep(100);
    }
  }
  assert(pids?.length === 2);
  await h.invoke('cancel_run', { id: slow.id });
  assert.equal((await done(slow.id)).s.status, 'interrupted');
  execFileSync(
    'powershell.exe',
    [
      '-NoProfile',
      '-Command',
      `if(Get-Process -Id ${pids.join(',')} -ErrorAction SilentlyContinue){exit 1}`,
    ],
    { windowsHide: true },
  );
  checks.push('Stop kills the owned Cursor process tree');
  await h.page.reload();
  await h.page.getByRole('button', { name: 'Settings', exact: true }).click();
  await h.page.getByRole('tab', { name: 'Accounts', exact: true }).click();
  await h.page.getByRole('button', { name: 'Add account', exact: true }).click();
  await h.page.getByRole('combobox', { name: 'Provider', exact: true }).click();
  await h.page.getByRole('option', { name: 'OpenCode', exact: true }).waitFor();
  await h.page.getByRole('option', { name: 'Cursor CLI', exact: true }).waitFor();
  await h.page.evaluate(() =>
    document
      .getAnimations()
      .filter((a) => Number.isFinite(a.effect.getComputedTiming().endTime))
      .forEach((a) => a.finish()),
  );
  assert.equal((await h.page.locator('.provider-avatar.cursor').textContent()).trim(), 'Cu');
  await h.page.screenshot({ path: '.codex/verification/081-cursor-account-picker.png' });
  checks.push('Native account picker exposes both OpenCode and Cursor');
  await writeFile(
    '.codex/verification/081-cursor-native.json',
    JSON.stringify(
      {
        passed: true,
        checks,
        scope:
          'Offline fixtures, actual Windows WebView2/IPC/storage/owned processes. No Cursor login or remote inference.',
      },
      null,
      2,
    ),
  );
  console.log(checks.join('\n'));
} finally {
  await h.close();
}
