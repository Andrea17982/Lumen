import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { nativeHarness } from './native-harness.mjs';
const h = await nativeHarness('09-antigravity');
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const checks = [];
try {
  const a = await h.invoke('add_account', { provider: 'antigravity', name: 'AG corpus' });
  await h.invoke('save_settings', {
    value: {
      ...(await h.invoke('get_settings')),
      verifyChanges: false,
      repoContext: false,
      orchestra: false,
    },
  });
  const opts = {
    mode: 'antigravity',
    accountId: a.id,
    model: 'gemini-3.8-flash-low',
    effort: 'low',
    accessMode: 'read-only',
    autoReview: false,
  };
  async function send(text) {
    const s = await h.invoke('create_session', { workspace: h.workspace });
    await h.invoke('send_message', { id: s.id, text, options: opts });
    return s.id;
  }
  async function done(id) {
    for (let i = 0; i < 200; i++) {
      const s = (await h.invoke('list_sessions', { search: '', offset: 0 })).find(
        (x) => x.id === id,
      );
      if (['completed', 'error', 'interrupted'].includes(s.status))
        return { s, m: (await h.invoke('get_messages', { id })).at(-1) };
      await sleep(50);
    }
    throw Error('AG fixture timed out');
  }
  const id = await send('AG_CORPUS_ERROR');
  const e = await done(id);
  assert.equal(e.s.status, 'error');
  assert(e.m.text.includes('Partial Antigravity work retained'));
  assert(e.m.activity.some((x) => x.kind === 'adapter-error'));
  checks.push(
    'Terminal native failure is an error despite exit code zero; partial text and typed failure survive',
  );
  const q = await send('AG_CORPUS_QUOTA');
  const quota = await done(q);
  assert.equal(quota.s.status, 'error');
  assert(quota.m.text.includes('Partial Antigravity work retained'));
  assert(!quota.m.activity.some((x) => x.kind === 'fallback'));
  checks.push('Quota after partial output never triggers an automatic replay or provider fallback');
  const s = await send('AG_CORPUS_SLOW');
  let pids;
  for (let i = 0; i < 150; i++) {
    try {
      pids = JSON.parse(await readFile(path.join(h.workspace, 'agy-owned-pids.json'), 'utf8'));
      break;
    } catch {}
    await sleep(50);
  }
  assert.equal(pids.length, 2);
  await h.invoke('cancel_run', { id: s });
  assert.equal((await done(s)).s.status, 'interrupted');
  for (let i = 0; i < 100; i++) {
    if (
      pids.every((pid) => {
        try {
          process.kill(pid, 0);
          return false;
        } catch {
          return true;
        }
      })
    )
      break;
    await sleep(50);
  }
  for (const pid of pids) assert.throws(() => process.kill(pid, 0));
  checks.push('Stop kills the actual Antigravity fixture parent and its owned child tree');
  const forbidden = await h.invoke('create_session', { workspace: h.workspace });
  await h.invoke('send_message', {
    id: forbidden.id,
    text: 'Ask fixture',
    options: { ...opts, accessMode: 'ask' },
  });
  const denied = await done(forbidden.id);
  assert.equal(denied.s.status, 'error');
  assert(!denied.m.text.includes('GOOGLE_REPLY_OK'));
  checks.push('Unsupported per-action Ask profile is rejected before native execution');
  await writeFile(
    '.codex/verification/09-antigravity-native.json',
    JSON.stringify(
      {
        passed: true,
        checks,
        scope: 'Windows IPC and offline official-stream fixtures; no real inference.',
      },
      null,
      2,
    ),
  );
  console.log(checks.join('\n'));
} finally {
  await h.close();
}
