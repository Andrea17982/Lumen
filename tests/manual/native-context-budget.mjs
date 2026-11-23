// Captures only harness-owned offline CLI inputs, never real provider history.
import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile, realpath } from 'node:fs/promises';
import path from 'node:path';
import { nativeHarness } from './native-harness.mjs';

const baseline = process.argv.includes('--baseline');
const h = await nativeHarness('context-budget');
const samples = [];
const checks = [];
try {
  const a = await h.invoke('add_account', { name: 'Context A', provider: 'claude' });
  const b = await h.invoke('add_account', { name: 'Context B', provider: 'claude' });
  const session = await h.invoke('create_session', { workspace: h.workspace });
  const file = path.join(h.workspace, 'contract.ts');
  await writeFile(file, 'UNIQUE_ATTACHED_CONTRACT: keep reset and keyboard access Ω.');
  const notes = (marker) =>
    `${'Project note. '.repeat(800)}\n${marker}: preserve the approved palette Ω.\n${'Project rule. '.repeat(800)}`;
  await writeFile(path.join(h.workspace, 'LUMEN.md'), notes('ORIGINAL_MIDDLE_CONTRACT'));
  async function turn(label, account, text, attachments = []) {
    await h.invoke('send_message', {
      id: session.id,
      text,
      options: {
        mode: 'claude',
        accountId: account.id,
        model: 'sonnet',
        effort: 'low',
        accessMode: 'autopilot',
        autoReview: false,
        attachments,
      },
    });
    let status;
    for (let i = 0; i < 300; i++) {
      status = (await h.invoke('list_sessions', { search: '', offset: 0 })).find(
        (row) => row.id === session.id,
      )?.status;
      if (['completed', 'error'].includes(status)) break;
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    assert.equal(
      status,
      'completed',
      JSON.stringify(await h.invoke('get_messages', { id: session.id })),
    );
    const rows = (
      await readFile(path.join(h.data, 'accounts', account.id, 'context-payloads.jsonl'), 'utf8')
    )
      .trim()
      .split(/\r?\n/)
      .map((line) => JSON.parse(line.replace(/^\uFEFF/, '')));
    const captured = rows.at(-1);
    const prompt = captured.prompt;
    samples.push({
      case: label,
      bytes: Buffer.byteLength(prompt),
      localTokens: await h.invoke('count_tokens', { text: prompt }),
      nativeResume: captured.resume === 'true',
      requirementCopies: prompt.split('OWNER_REQUEST_CONTRACT').length - 1,
      attachmentCopies: prompt.split('UNIQUE_ATTACHED_CONTRACT').length - 1,
    });
    return captured;
  }
  const first = await turn(
    'initial',
    a,
    `Mantieni OWNER_REQUEST_CONTRACT: reset and palette are approved. PROMPT_BUDGET_FIXTURE ${'Concrete implementation requirement. '.repeat(25)}`,
    [file, file],
  );
  assert(first.prompt.includes('ORIGINAL_MIDDLE_CONTRACT'));
  assert.equal(samples.at(-1).attachmentCopies, baseline ? 2 : 1);
  assert.equal(samples.at(-1).requirementCopies, baseline ? 4 : 2);
  checks.push(
    'Initial CLI payload preserves owner requirements, Unicode and project notes; exact duplicates measured',
  );
  const unchanged = await turn(
    'unchanged native continuation',
    a,
    'Continue PROMPT_BUDGET_FIXTURE',
  );
  assert.equal(unchanged.resume, 'true');
  assert(!unchanged.prompt.includes('ORIGINAL_MIDDLE_CONTRACT'));
  checks.push('Unchanged notes/history are not resent on native continuation');
  await writeFile(path.join(h.workspace, 'LUMEN.md'), notes('UPDATED_MIDDLE_CONTRACT'));
  const changed = await turn(
    'updated notes native continuation',
    a,
    'Continue PROMPT_BUDGET_FIXTURE',
  );
  assert.equal(changed.prompt.includes('UPDATED_MIDDLE_CONTRACT'), !baseline);
  assert(!changed.prompt.includes('ORIGINAL_MIDDLE_CONTRACT'));
  checks.push(
    baseline
      ? 'Baseline reproduces omitted middle requirement in changed notes'
      : 'Changed notes reach the resumed CLI including the middle requirement',
  );
  const repeated = await turn('unchanged after update', a, 'Continue PROMPT_BUDGET_FIXTURE');
  assert(!repeated.prompt.includes('UPDATED_MIDDLE_CONTRACT'));
  const handed = await turn('account handoff', b, 'Continue PROMPT_BUDGET_FIXTURE');
  assert.equal(handed.resume, 'false');
  assert(handed.prompt.includes('OWNER_REQUEST_CONTRACT'));
  assert(handed.prompt.includes('UPDATED_MIDDLE_CONTRACT'));
  checks.push('Account handoff rebuilds current owner context without old project notes');
  const other = path.join(h.data, 'other-workspace');
  await mkdir(other);
  await writeFile(path.join(other, 'LUMEN.md'), 'OTHER_WORKSPACE_REQUIREMENT');
  await h.invoke('set_session_workspace', { id: session.id, folder: other, workspaceId: null });
  const switched = await turn('workspace switch', b, 'Continue PROMPT_BUDGET_FIXTURE');
  assert.equal(switched.resume, 'false');
  assert.equal(await realpath(switched.workspace), await realpath(other));
  assert(switched.prompt.includes('OTHER_WORKSPACE_REQUIREMENT'));
  assert(!switched.prompt.includes('UPDATED_MIDDLE_CONTRACT'));
  checks.push(
    'Explicit directory change starts a full invocation in the new directory with only its project notes',
  );
  await mkdir('.impeccable/context-budget', { recursive: true });
  const report = { baseline, offline: true, liveCalls: 0, samples, checks };
  await writeFile(
    `.impeccable/context-budget/native-${baseline ? 'baseline' : 'candidate'}.json`,
    JSON.stringify(report, null, 2),
  );
  console.log(JSON.stringify(report, null, 2));
} finally {
  await h.close();
}
