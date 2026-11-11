// Controlled prompt delivery and account/routing acceptance; no model-quality or live claim.
import assert from 'node:assert/strict';
import { writeFile, mkdir } from 'node:fs/promises';
import path from 'node:path';
import { nativeHarness } from './native-harness.mjs';
import { appendValidation } from './quality-evidence.mjs';

const h = await nativeHarness('quality');
const checks = [];
try {
  const a = await h.invoke('add_account', { name: 'Claude A', provider: 'claude' });
  const b = await h.invoke('add_account', { name: 'Claude B', provider: 'claude' });
  const g = await h.invoke('add_account', { name: 'GPT', provider: 'gpt' });
  const s = await h.invoke('create_session', { workspace: h.workspace });
  const options = (account) => ({
    mode: account.provider,
    accountId: account.id,
    model: account.provider === 'claude' ? 'sonnet' : 'gpt-fixture',
    effort: 'low',
    accessMode: 'autopilot',
  });
  async function turn(session, account, text, extra = {}) {
    const plan = await h.invoke('send_message', {
      id: session.id,
      text,
      options: { ...options(account), ...extra },
    });
    for (let i = 0; i < 250; i++) {
      const state = (await h.invoke('list_sessions', { search: '', offset: 0 })).find(
        (row) => row.id === session.id,
      )?.status;
      if (state === 'completed')
        return { plan, messages: await h.invoke('get_messages', { id: session.id }) };
      if (state === 'error')
        throw new Error(
          JSON.stringify((await h.invoke('get_messages', { id: session.id })).at(-1)),
        );
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    throw new Error('Offline turn did not finish');
  }
  const native = (result) =>
    JSON.parse(result.messages.at(-1).activity.findLast((row) => row.kind === 'native').text);
  const first = await turn(
    s,
    a,
    'Maintain the limit of 3 across all account handoffs. QUALITY_CONTEXT_FIXTURE',
  );
  const initial = native(first);
  assert.equal(initial.account, a.id);
  h.validationWorkspace = s.workspace;
  appendValidation(
    h,
    s.id,
    'Independent counter check',
    'failed',
    'UI must disable Hit at 3; preserve label/reset.',
  );
  const resumed = await turn(s, a, 'QUALITY_CONTEXT_FIXTURE');
  assert(resumed.messages.at(-1).text.includes('QUALITY_CONTEXT_PRESENT'));
  assert.equal(native(resumed).session, initial.session);
  const unchanged = await turn(s, a, 'QUALITY_CONTEXT_FIXTURE');
  assert(unchanged.messages.at(-1).text.includes('QUALITY_CONTEXT_ABSENT'));
  assert.equal(typeof native(resumed).context_revision, 'string');
  assert.equal(native(unchanged).context_revision, native(resumed).context_revision);
  checks.push(
    'Same-account native continuation receives changed backend check evidence once, not every turn',
  );
  for (const account of [b, g]) {
    const handed = await turn(s, account, 'QUALITY_CONTEXT_FIXTURE');
    assert(handed.messages.at(-1).text.includes('QUALITY_CONTEXT_PRESENT'));
    assert.equal(native(handed).account, account.id);
  }
  checks.push(
    'Two Claude accounts and GPT share current chat facts through full handoff without sharing native identities',
  );
  const other = await h.invoke('create_session', { workspace: h.workspace });
  const isolated = await turn(other, g, 'QUALITY_CONTEXT_FIXTURE');
  assert(isolated.messages.at(-1).text.includes('QUALITY_CONTEXT_ABSENT'));
  checks.push('Check context never leaks into a different chat using the same project/account');

  for (const account of [a, g]) {
    const chat = await h.invoke('create_session', { workspace: h.workspace });
    await turn(chat, account, 'Keep the counter limit of 3. QUALITY_CONTEXT_FIXTURE');
    appendValidation(h, chat.id, 'Independent counter check', 'failed', 'Current task check');
    const changed = await turn(chat, account, 'New task: QUALITY_CONTEXT_FIXTURE');
    assert(changed.messages.at(-1).text.includes('QUALITY_CONTEXT_ABSENT'));
    const target = changed.messages.findLast((message) => message.role === 'user');
    await h.invoke('rewind', { session: chat.id, message: target.id });
    const retained = await h.invoke('get_messages', { id: chat.id });
    assert(retained.every((message) => message.activity.every((row) => row.kind !== 'native')));
    assert(retained.some((message) => message.activity.some((row) => row.kind === 'validation')));
    const rebuilt = await turn(chat, account, 'QUALITY_CONTEXT_FIXTURE');
    assert(rebuilt.messages.at(-1).text.includes('QUALITY_CONTEXT_PRESENT'));
    assert.equal(native(rebuilt).account, account.id);
    checks.push(
      `${account.provider}: explicit new task retires old evidence; Rewind clears native IDs and rebuilds from retained checks`,
    );
  }

  const balanced = await h.invoke('create_session', { workspace: h.workspace });
  await turn(balanced, a, 'Start this conversation.');
  const at = Date.now();
  const usages = [a, b, g].map((account, index) => ({
    accountId: account.id,
    updatedAt: at,
    source: 'controlled fixture',
    windows: [{ usedPercent: [95, 5, 100][index], windowMinutes: 300, resetsAt: at + 3600000 }],
  }));
  await writeFile(path.join(h.data, 'usage.json'), JSON.stringify(usages));
  const chosen = await turn(balanced, b, 'Ciao', {
    mode: 'auto',
    accountId: null,
    model: null,
    balanceUsage: true,
    autoPolicy: 'default',
  });
  assert.equal(chosen.plan.accountId, b.id);
  checks.push('Fresh quota balancing is not undone by the previous native account');

  // Restore controlled quota after the native fixture's own usage event.
  await writeFile(path.join(h.data, 'usage.json'), JSON.stringify(usages));
  const reviewed = await turn(balanced, b, 'Fai un cross-audit di questo fix', {
    mode: 'auto',
    accountId: null,
    model: null,
    balanceUsage: true,
    autoPolicy: 'budget',
  });
  assert.equal(reviewed.plan.strategy, 'review');
  assert.equal(reviewed.plan.accountId, b.id);
  assert.equal(reviewed.plan.partnerAccount, a.id);
  assert.equal(native(reviewed).account, b.id);
  assert(
    reviewed.messages
      .at(-1)
      .activity.some((row) => row.kind === 'review' && row.text.includes('Claude A')),
  );
  checks.push(
    'Budget honors an explicit audit on another Claude account when GPT quota is unavailable',
  );
  await writeFile(
    path.join(h.data, 'usage.json'),
    JSON.stringify(
      usages.map((usage, index) => ({
        ...usage,
        windows: [{ ...usage.windows[0], usedPercent: index === 0 ? 0 : 100 }],
      })),
    ),
  );
  const beforeUnavailable = (await h.invoke('get_messages', { id: balanced.id })).length;
  await assert.rejects(
    () =>
      h.invoke('send_message', {
        id: balanced.id,
        text: 'Fai un cross-audit di questo fix',
        options: {
          ...options(a),
          mode: 'auto',
          accountId: null,
          model: null,
          balanceUsage: true,
          autoPolicy: 'budget',
        },
      }),
    /Independent review is unavailable/,
  );
  assert.equal((await h.invoke('get_messages', { id: balanced.id })).length, beforeUnavailable);
  checks.push('A quota-unavailable explicit audit is rejected before writing or claiming a pass');
  await writeFile(path.join(h.data, 'usage.json'), '[]');
  await writeFile(path.join(h.data, 'accounts', a.id, 'quality-rate-once'), 'offline fixture only');
  const fallbackChat = await h.invoke('create_session', { workspace: h.workspace });
  const fallbackReviewed = await turn(
    fallbackChat,
    a,
    'Fix this React CSS component with an independent review',
    { autoReview: true },
  );
  assert.equal(native(fallbackReviewed).account, g.id);
  const fallbackActivities = fallbackReviewed.messages.at(-1).activity;
  assert(fallbackActivities.some((row) => row.kind === 'review' && row.text.includes('Claude B')));
  assert(fallbackActivities.findLast((row) => row.kind === 'model').text.includes('Sonnet'));
  checks.push(
    'Writer quota fallback selects an independent reviewer and its proportional catalog model while preserving the actual writer identity',
  );
  await mkdir(path.join(h.root, '.impeccable/review'), { recursive: true });
  await writeFile(
    path.join(h.root, '.impeccable/review/native-quality-results.json'),
    JSON.stringify(
      {
        at: new Date().toISOString(),
        passed: true,
        checks,
        scope:
          'Windows debug; real IPC/SQLite, controlled account homes and offline CLI fixtures. Prompt/account correctness, not live code-quality evidence.',
      },
      null,
      2,
    ),
  );
  process.stdout.write(JSON.stringify({ passed: true, checks }, null, 2) + '\n');
} finally {
  await h.close();
}
