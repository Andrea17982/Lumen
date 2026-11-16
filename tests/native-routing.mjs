import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { nativeHarness } from './native-harness.mjs';

const h = await nativeHarness('routing');
try {
  const claude = await h.invoke('add_account', { name: 'Claude', provider: 'claude' });
  await h.invoke('add_account', { name: 'GPT', provider: 'gpt' });
  const google = await h.invoke('add_account', { name: 'Google', provider: 'antigravity' });
  const models = await h.invoke('list_models', { id: google.id });
  assert.deepEqual(
    models.map((m) => m.id),
    ['gemini-3.8-flash-low'],
  );
  const questions = [];
  async function run(text, options, answer = 'Continue') {
    const chat = await h.invoke('create_session', { workspace: h.workspace });
    const plan = await h.invoke('send_message', { id: chat.id, text, options });
    const deadline = Date.now() + 60000;
    while (Date.now() < deadline) {
      const messages = await h.invoke('get_messages', { id: chat.id });
      const pending = await h.invoke('pending_questions');
      for (const row of messages.at(-1)?.activity ?? []) {
        if (row.kind !== 'interaction') continue;
        const request = JSON.parse(row.text);
        if (!pending.includes(request.id)) continue;
        questions.push(request.questions[0].question);
        await h.invoke('answer_question', { id: request.id, answers: { switch: [answer] } });
      }
      const session = (await h.invoke('list_sessions', { search: '', offset: 0 })).find(
        (s) => s.id === chat.id,
      );
      if (['completed', 'error', 'interrupted'].includes(session.status))
        return { plan, session, message: messages.at(-1) };
      await new Promise((r) => setTimeout(r, 100));
    }
    throw Error('Routing fixture timed out');
  }
  const explicit = await run('Usa antigravity per questo prompt: descrivi il progetto.', {
    mode: 'auto',
    effort: '',
    accessMode: 'read-only',
  });
  assert.equal(explicit.plan.provider, 'antigravity');
  assert.equal(explicit.session.status, 'completed');
  assert.equal(explicit.message.text, 'GOOGLE_REPLY_OK');
  const chain = await run('QUOTA_CHAIN: implement a security change', {
    mode: 'claude',
    accountId: claude.id,
    model: 'sonnet',
    effort: 'low',
    accessMode: 'autopilot',
  });
  assert.equal(chain.session.status, 'completed', JSON.stringify(chain.message));
  assert.equal(chain.message.text, 'GOOGLE_REPLY_OK');
  assert.equal(
    JSON.parse(chain.message.activity.find((a) => a.kind === 'usage').text).provider,
    'antigravity',
  );
  assert.equal(questions.length, 2);
  assert(questions[1].includes('drop in quality'));
  assert.deepEqual(
    (await readFile(path.join(h.workspace, 'quota-chain.txt'), 'utf8')).trim().split(/\r?\n/),
    ['claude', 'gpt', 'antigravity'],
  );
  assert.equal(chain.message.activity.filter((a) => a.kind === 'fallback').length, 2);
  assert.equal((await h.invoke('pending_questions')).length, 0);
  const rejected = await run(
    'QUOTA_CHAIN: stop instead of changing provider',
    {
      mode: 'claude',
      accountId: claude.id,
      model: 'sonnet',
      effort: 'low',
      accessMode: 'autopilot',
    },
    'Stop',
  );
  assert.equal(rejected.session.status, 'interrupted');
  assert.equal((await h.invoke('pending_questions')).length, 0);
  assert.deepEqual(
    (await readFile(path.join(h.workspace, 'quota-chain.txt'), 'utf8')).trim().split(/\r?\n/),
    ['claude', 'gpt', 'antigravity', 'claude'],
  );
  console.log(
    'Passed: explicit Auto provider, Google-only catalog, three-account credit fallback with two approvals and quality warning, Antigravity stream completion. No live inference.',
  );
} finally {
  await h.close();
}
