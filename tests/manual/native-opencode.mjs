// Real ACP client, policy API/child process fixtures, native Lumen MCP helpers. No inference.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile, writeFile, mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { nativeHarness } from './native-harness.mjs';
const nonce = 'opencode-' + crypto.randomUUID();
const web = createServer((req, res) => {
  res.setHeader('Content-Type', 'text/html');
  res.end(`<html><body>${nonce}</body></html>`);
});
await new Promise((r) => web.listen(0, '127.0.0.1', r));
const discovery = await mkdtemp(path.join(tmpdir(), 'lumen-opencode-discovery-'));
const h = await nativeHarness('09-opencode', {
  appEnv: {
    LUMEN_DISCOVERY_HOME: discovery,
    LUMEN_DISCOVERY_CLAUDE_HOME: discovery,
    LUMEN_DISCOVERY_CODEX_HOME: discovery,
  },
});
const checks = [];
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function done(id) {
  for (let i = 0; i < 500; i++) {
    const s = (await h.invoke('list_sessions', { search: '', offset: 0 })).find((x) => x.id === id);
    if (['completed', 'error', 'interrupted'].includes(s.status))
      return { s, m: (await h.invoke('get_messages', { id })).at(-1) };
    await sleep(50);
  }
  throw Error('Native OpenCode turn did not finish');
}
async function pending(id) {
  for (let i = 0; i < 200; i++) {
    const m = (await h.invoke('get_messages', { id })).at(-1);
    const row = m?.activity.find((x) => x.kind === 'interaction');
    if (row) return JSON.parse(row.text);
    await sleep(50);
  }
  throw Error('No OpenCode permission question');
}
let a;
async function send(text, accessMode) {
  const s = await h.invoke('create_session', { workspace: h.workspace });
  await h.invoke('send_message', {
    id: s.id,
    text,
    options: {
      mode: 'opencode',
      accountId: a.id,
      model: 'openai/gpt-6.1-sol',
      effort: '',
      accessMode,
      autoReview: false,
    },
  });
  return s;
}
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
  a = await h.invoke('add_account', { provider: 'opencode', name: 'OpenCode ACP fixture' });
  const readonly = await send('OPENCODE_WRITE', 'read-only');
  const denied = await done(readonly.id);
  assert.equal(denied.m.text, 'WRITE_DENIED');
  await assert.rejects(readFile(path.join(h.workspace, 'opencode-owned-write.txt')));
  const edits = await send('OPENCODE_WRITE', 'file-edits');
  assert.equal((await done(edits.id)).m.text, 'WRITE_OK');
  assert.equal(
    await readFile(path.join(h.workspace, 'opencode-owned-write.txt'), 'utf8'),
    'once\n',
  );
  checks.push(
    'Verified session-scoped policy before prompt; Read only denies a forced write, File edits permits an internal write',
  );
  const ask = await send('OPENCODE_WRITE', 'ask');
  const q = await pending(ask.id);
  assert.equal(q.kind, 'permission');
  await h.invoke('answer_question', { id: q.id, answers: { action: ['Deny'] } });
  assert.equal((await done(ask.id)).m.text, 'WRITE_DENIED');
  const once = await send('OPENCODE_WRITE', 'ask');
  const allow = await pending(once.id);
  assert.notEqual(q.id, allow.id);
  await h.invoke('answer_question', { id: allow.id, answers: { action: ['Allow once'] } });
  assert.equal((await done(once.id)).m.text, 'WRITE_OK');
  assert.equal(
    await readFile(path.join(h.workspace, 'opencode-owned-write.txt'), 'utf8'),
    'once\nonce\n',
  );
  const stop = await send('OPENCODE_WRITE', 'ask');
  const stale = await pending(stop.id);
  await h.invoke('cancel_run', { id: stop.id });
  assert.equal((await done(stop.id)).s.status, 'interrupted');
  await assert.rejects(
    h.invoke('answer_question', { id: stale.id, answers: { action: ['Allow once'] } }),
  );
  checks.push(
    'One-shot ACP permission decisions, rejection, cancellation and stale-answer expiry without permanent grants',
  );
  await writeFile(
    path.join(h.workspace, 'fixture.ts'),
    `export function opencode_graph_${nonce.replaceAll('-', '_')}(){return 1}`,
  );
  const graph = await send('OPENCODE_GRAPH', 'read-only');
  assert((await done(graph.id)).m.text.includes('opencode_graph_'));
  const browser = await send(
    `OPENCODE_BROWSER_URL=http://127.0.0.1:${web.address().port}/`,
    'read-only',
  );
  assert((await done(browser.id)).m.text.includes(nonce));
  checks.push(
    'Actual Lumen graph and Moli MCP helpers receive invocation-scoped config through ACP and return independent oracles',
  );
  const quota = await send('OPENCODE_QUOTA', 'autopilot');
  const failed = await done(quota.id);
  assert.equal(failed.s.status, 'error');
  assert(failed.m.text.includes('Partial work retained'));
  assert(failed.m.activity.some((x) => x.kind === 'native'));
  const effects = await readFile(path.join(h.workspace, 'opencode-effects.txt'), 'utf8');
  await h.invoke('resume_at_reset', { id: quota.id, message: failed.m.id });
  const resumed = await done(quota.id);
  assert.equal(resumed.s.status, 'completed');
  assert.equal(resumed.m.text, 'OPENCODE_RESUME_OK');
  assert.equal(await readFile(path.join(h.workspace, 'opencode-effects.txt'), 'utf8'), effects);
  assert(!JSON.stringify(resumed.m).includes('OLD_HISTORY_SHOULD_NOT_REPLAY'));
  checks.push(
    'ACP load replay is ignored; quota preserves native identity/partial output and scheduled continuation does not repeat the file effect',
  );
  await writeFile(
    '.codex/verification/09-opencode-native.json',
    JSON.stringify(
      {
        passed: true,
        checks,
        scope:
          'Offline OpenCode ACP/policy fixtures with actual Windows IPC, storage and real Lumen graph/browser MCP helpers; no real inference.',
      },
      null,
      2,
    ),
  );
  console.log(checks.join('\n'));
} finally {
  await h.close();
  web.closeAllConnections();
  await new Promise((r) => web.close(r));
}
