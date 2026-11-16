// Isolated real IPC/SQLite/native processes; deterministic offline CLI fixtures, no inference.
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { nativeHarness } from './native-harness.mjs';
const h = await nativeHarness('081-continuity');
const checks = [];
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function done(id) {
  for (let i = 0; i < 600; i++) {
    const s = (await h.invoke('list_sessions', { search: '', offset: 0 })).find((s) => s.id === id);
    if (['completed', 'error', 'interrupted'].includes(s.status))
      return { s, m: (await h.invoke('get_messages', { id })).at(-1) };
    await sleep(100);
  }
  throw Error('Turn did not finish');
}
async function chat() {
  return h.invoke('create_session', { workspace: h.workspace });
}
const options = (mode, id, model) => ({
  mode,
  accountId: id,
  model,
  effort: mode === 'claude' ? 'high' : '',
  accessMode: 'autopilot',
  autoReview: false,
});
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
  const claude = await h.invoke('add_account', { name: 'Claude fixture', provider: 'claude' });
  await h.invoke('add_account', { name: 'GPT fixture', provider: 'gpt' });
  const original = await chat();
  await h.invoke('send_message', {
    id: original.id,
    text: 'QUOTA_GUARD_TOOL: implement the next release block and build the app.',
    options: options('claude', claude.id, 'opus'),
  });
  const failed = await done(original.id);
  assert.equal(failed.s.status, 'error');
  assert(failed.m.activity.some((a) => a.kind === 'quota-reset'));
  const continuation = await h.invoke('send_message', {
    id: original.id,
    text: 'Prosegui.',
    options: { mode: 'auto', effort: '', accessMode: 'autopilot', autoReview: false },
  });
  assert.equal(continuation.provider, 'gpt');
  assert.equal(continuation.model, 'gpt-fixture');
  assert.equal(continuation.effort, 'high');
  assert.equal((await done(original.id)).s.status, 'completed');
  checks.push(
    'Manual Opus + quota + Prosegui preserves flagship class/high reasoning and chooses GPT Sol fixture',
  );
  await h.invoke('save_settings', {
    value: { ...(await h.invoke('get_settings')), autoFallback: false },
  });
  const gpt = (await h.invoke('list_accounts')).find((a) => a.provider === 'gpt');
  const exhausted = await chat();
  await h.invoke('send_message', {
    id: exhausted.id,
    text: 'QUOTA_CHAIN',
    options: options('gpt', gpt.id, 'gpt-fixture'),
  });
  assert.equal((await done(exhausted.id)).s.status, 'error');
  await h.invoke('add_account', { name: 'Google fixture', provider: 'antigravity' });
  const weak = await h.invoke('send_message', {
    id: exhausted.id,
    text: 'Prosegui.',
    options: { mode: 'auto', effort: '', accessMode: 'autopilot', autoReview: false },
  });
  assert.equal(weak.provider, 'antigravity');
  let question;
  for (let i = 0; i < 100; i++) {
    const last = (await h.invoke('get_messages', { id: exhausted.id })).at(-1);
    const row = last.activity.find((a) => a.kind === 'interaction');
    if (row) {
      question = JSON.parse(row.text);
      break;
    }
    await sleep(100);
  }
  assert(question.questions[0].question.includes('weaker'));
  await h.invoke('answer_question', { id: question.id, answers: { switch: ['Stop'] } });
  assert.equal((await done(exhausted.id)).s.status, 'interrupted');
  assert.equal((await readFile(path.join(h.workspace, 'quota-chain.txt'), 'utf8')).trim(), 'gpt');
  await h.invoke('save_settings', {
    value: { ...(await h.invoke('get_settings')), autoFallback: true },
  });
  checks.push(
    'Only a weaker model remains: Auto asks before any inference, and Stop prevents the send',
  );
  const retry = await chat();
  await h.invoke('send_message', {
    id: retry.id,
    text: 'QUOTA_RESET_FIXTURE: create the one requested effect, then finish.',
    options: options('claude', claude.id, 'opus'),
  });
  const interrupted = await done(retry.id);
  assert.equal(interrupted.s.status, 'error');
  await h.invoke('resume_at_reset', { id: retry.id, message: interrupted.m.id });
  const resumed = await done(retry.id);
  assert.equal(resumed.s.status, 'completed', JSON.stringify(resumed.m));
  assert(resumed.m.text.includes('RESUME_AFTER_QUOTA_OK'));
  assert.equal(
    (await readFile(path.join(h.workspace, 'quota-reset-effect.txt'), 'utf8')).trim(),
    'once',
  );
  checks.push(
    'Reset schedule resumes native session exactly once without repeating the existing file effect',
  );
  const missing = await chat();
  await h.invoke('send_message', {
    id: missing.id,
    text: 'QUOTA_RESET_FIXTURE QUOTA_RESET_MISSING: perform work.',
    options: options('claude', claude.id, 'opus'),
  });
  const lost = await done(missing.id);
  await h.invoke('resume_at_reset', { id: missing.id, message: lost.m.id });
  assert.equal((await done(missing.id)).s.status, 'error');
  assert.deepEqual(
    (await readFile(path.join(h.workspace, 'quota-reset-attempts.txt'), 'utf8'))
      .trim()
      .split(/\r?\n/),
    ['initial', 'resume', 'initial', 'resume'],
  );
  checks.push('Missing native session after scheduled reset fails without a fresh replay');
  const stopped = await chat();
  await h.invoke('send_message', {
    id: stopped.id,
    text: 'QUOTA_RESET_FIXTURE QUOTA_RESET_CANCEL: keep the work.',
    options: options('claude', claude.id, 'opus'),
  });
  const stopReply = await done(stopped.id);
  await h.invoke('resume_at_reset', { id: stopped.id, message: stopReply.m.id });
  await h.invoke('cancel_run', { id: stopped.id });
  assert.equal((await done(stopped.id)).s.status, 'interrupted');
  await sleep(5500);
  assert.equal(
    (await readFile(path.join(h.workspace, 'quota-reset-attempts.txt'), 'utf8'))
      .trim()
      .split(/\r?\n/).length,
    5,
  );
  checks.push('Cancel scheduled resumption prevents the future request');
  const open = await h.invoke('add_account', { name: 'OpenCode fixture', provider: 'opencode' });
  await assert.rejects(
    h.invoke('add_account', { name: 'Duplicate profile', provider: 'opencode' }),
  );
  const models = await h.invoke('list_models', { id: open.id });
  assert.equal(models[0].id, 'openai/gpt-6.1-sol');
  const oc = await chat();
  await h.invoke('send_message', {
    id: oc.id,
    text: 'Write the fixture result.',
    options: options('opencode', open.id, models[0].id),
  });
  assert((await done(oc.id)).m.text.includes('OPENCODE_FIRST_OK'));
  await h.invoke('send_message', {
    id: oc.id,
    text: 'Prosegui.',
    options: options('opencode', open.id, models[0].id),
  });
  assert((await done(oc.id)).m.text.includes('OPENCODE_RESUME_OK'));
  const restricted = await chat();
  await h.invoke('send_message', {
    id: restricted.id,
    text: 'Read only probe.',
    options: { ...options('opencode', open.id, models[0].id), accessMode: 'read-only' },
  });
  assert.equal((await done(restricted.id)).s.status, 'completed');
  assert.equal(
    (await readFile(path.join(h.workspace, 'opencode-effects.txt'), 'utf8')).trim().split(/\r?\n/)
      .length,
    2,
  );
  checks.push(
    'OpenCode discovery, native continuation, single profile and Read only without project writes',
  );
  const slow = await chat();
  await h.invoke('send_message', {
    id: slow.id,
    text: 'OPENCODE_SLOW_FIXTURE',
    options: options('opencode', open.id, models[0].id),
  });
  let pids;
  for (let i = 0; i < 100; i++) {
    try {
      pids = JSON.parse(await readFile(path.join(h.workspace, 'opencode-pids.json'), 'utf8'));
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
  checks.push('OpenCode cancellation kills its owned child tree');
  await writeFile(
    '.codex/verification/081-native.json',
    JSON.stringify(
      { passed: true, checks, scope: 'Offline fixtures. No live inference or real credentials.' },
      null,
      2,
    ),
  );
  console.log(checks.join('\n'));
} finally {
  await h.close();
}
