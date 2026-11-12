// Actual owned effects, disposable Windows profile and offline provider protocols.
import assert from 'node:assert/strict';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { nativeHarness } from './native-harness.mjs';

const h = await nativeHarness('quota-replay');
const checks = [];
try {
  const writer = await h.invoke('add_account', { name: 'Quota writer', provider: 'claude' });
  const alternative = await h.invoke('add_account', { name: 'Available GPT', provider: 'gpt' });
  async function turn(marker, existing) {
    const chat = existing || (await h.invoke('create_session', { workspace: h.workspace }));
    await h.invoke('send_message', {
      id: chat.id,
      text: marker,
      options: {
        mode: 'claude',
        accountId: writer.id,
        model: 'sonnet',
        effort: 'low',
        accessMode: 'autopilot',
      },
    });
    for (let i = 0; i < 300; i++) {
      const session = (await h.invoke('list_sessions', { search: '', offset: 0 })).find(
        (row) => row.id === chat.id,
      );
      if (['completed', 'error', 'interrupted'].includes(session?.status))
        return {
          chat,
          status: session.status,
          message: (await h.invoke('get_messages', { id: chat.id })).at(-1),
        };
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    throw new Error('Offline quota recovery timed out');
  }
  const lines = async (file) =>
    (await readFile(path.join(h.workspace, file), 'utf8')).trim().split(/\r?\n/);
  const tool = await turn('QUOTA_GUARD_TOOL');
  assert.equal(
    (await lines('quota-effect.txt')).length,
    1,
    'Quota fallback duplicated an already observed filesystem effect',
  );
  assert.equal(tool.status, 'error');
  assert.deepEqual(await lines('quota-attempt-tool.txt'), ['claude']);
  assert(!tool.message.activity.some((row) => row.kind === 'fallback'));
  assert(
    tool.message.activity.some(
      (row) => row.kind === 'error' && row.text.includes('Lumen did not replay'),
    ),
  );
  checks.push(
    'Quota after observed tool work retains the error and one owned write without switching accounts',
  );
  const partial = await turn('QUOTA_GUARD_PARTIAL');
  assert.equal(partial.status, 'error');
  assert.equal(partial.message.text, 'Preserve this partial result.');
  assert.deepEqual(await lines('quota-attempt-partial.txt'), ['claude']);
  checks.push(
    'Partial assistant output survives a quota error instead of being cleared for replay',
  );
  const primed = await turn('Start the nested native recovery test.');
  assert.equal(primed.status, 'completed');
  const nested = await turn('QUOTA_GUARD_NESTED', primed.chat);
  assert.equal(nested.status, 'error');
  assert.deepEqual(await lines('quota-attempt-nested.txt'), ['claude', 'claude']);
  assert.deepEqual(await lines('quota-effect-nested.txt'), ['claude']);
  assert(!nested.message.activity.some((row) => row.kind === 'fallback'));
  checks.push(
    'Missing-session recovery followed by work and quota shares the guard across both attempts',
  );
  const safe = await turn('QUOTA_GUARD_SAFE');
  assert.equal(safe.status, 'completed');
  assert.deepEqual(await lines('quota-attempt-safe.txt'), ['claude', 'gpt']);
  assert.equal(safe.message.text, 'QUOTA_FALLBACK_RECOVERED');
  const native = JSON.parse(safe.message.activity.findLast((row) => row.kind === 'native').text);
  assert.equal(native.account, alternative.id);
  checks.push(
    'Quota before observed work keeps one account switch and the actual fallback identity',
  );
  const output = path.join(h.root, '.impeccable/review');
  await mkdir(output, { recursive: true });
  await writeFile(
    path.join(output, 'native-quota-replay-results.json'),
    JSON.stringify(
      {
        passed: true,
        checks,
        scope:
          'Windows debug WebView2, real IPC/SQLite, owned temporary effects, offline Claude/GPT fixtures; no live inference.',
      },
      null,
      2,
    ),
  );
  process.stdout.write(JSON.stringify({ passed: true, checks }, null, 2) + '\n');
} finally {
  await h.close();
}
