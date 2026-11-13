// Offline native recovery with actual owned filesystem effects, never user projects.
import assert from 'node:assert/strict';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { nativeHarness } from './native-harness.mjs';

const h = await nativeHarness('resume-replay');
const checks = [];
try {
  async function turn(chat, account, text) {
    await h.invoke('send_message', {
      id: chat.id,
      text,
      options: {
        mode: account.provider,
        accountId: account.id,
        model: account.provider === 'claude' ? 'sonnet' : 'gpt-fixture',
        effort: 'low',
        accessMode: 'autopilot',
      },
    });
    for (let i = 0; i < 300; i++) {
      const session = (await h.invoke('list_sessions', { search: '', offset: 0 })).find(
        (s) => s.id === chat.id,
      );
      if (['completed', 'error', 'interrupted'].includes(session?.status)) {
        return {
          status: session.status,
          messages: await h.invoke('get_messages', { id: chat.id }),
        };
      }
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    throw new Error('Offline replay fixture timed out');
  }
  async function count(file) {
    return (await readFile(path.join(h.workspace, file), 'utf8')).trim().split(/\r?\n/).length;
  }
  for (const provider of ['claude', 'gpt']) {
    const account = await h.invoke('add_account', { name: `${provider} replay fixture`, provider });
    const chat = await h.invoke('create_session', { workspace: h.workspace });
    assert.equal(
      (await turn(chat, account, 'Start an isolated recovery test.')).status,
      'completed',
    );
    const failed = await turn(chat, account, 'REPLAY_TOOL_FIXTURE');
    assert.equal(failed.status, 'error');
    assert(
      failed.messages
        .at(-1)
        .activity.some((row) => row.kind === 'error' && row.text.includes('Lumen did not replay')),
    );
    assert.equal(
      await count(`replay-tool-${provider}.txt`),
      1,
      `${provider}: automatic replay duplicated a filesystem effect before any assistant prose`,
    );
    assert(!failed.messages.at(-1).activity.some((row) => row.kind === 'native'));
    checks.push(
      `${provider}: tool activity followed by a missing-session error is not replayed; one owned write`,
    );
    const missing = await h.invoke('create_session', { workspace: h.workspace });
    assert.equal(
      (await turn(missing, account, 'Start an isolated missing-session test.')).status,
      'completed',
    );
    const recovered = await turn(missing, account, 'REPLAY_MISSING_FIXTURE');
    assert.equal(recovered.status, 'completed');
    assert.equal(await count(`replay-missing-${provider}.txt`), 2);
    assert(recovered.messages.at(-1).text.includes('REPLAY_RECOVERED'));
    checks.push(
      `${provider}: missing native session before observed work keeps the single full-context recovery`,
    );
  }
  const output = path.join(h.root, '.impeccable/review');
  await mkdir(output, { recursive: true });
  await writeFile(
    path.join(output, 'native-resume-replay-results.json'),
    JSON.stringify(
      {
        passed: true,
        checks,
        scope:
          'Windows debug WebView2, real IPC/SQLite and owned temporary effects, offline CLI fixtures; no inference.',
      },
      null,
      2,
    ),
  );
  process.stdout.write(JSON.stringify({ passed: true, checks }, null, 2) + '\n');
} finally {
  await h.close();
}
