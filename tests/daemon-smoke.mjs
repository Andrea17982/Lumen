import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import { spawn, execFileSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
import net from 'node:net';
import { daemonClient, sleep } from './daemon-harness.mjs';

const data = await mkdtemp(path.join(tmpdir(), 'lumen-daemon-'));
const workspace = path.join(data, 'workspace');
await mkdir(workspace);
const client = await daemonClient(data);
const daemon = client.start();
let log = '';
daemon.stderr.on('data', (b) => {
  log += b;
});
let stream;
let checkQuotaReplay;
try {
  assert.equal((await client.ready()).idle, true, log);
  // Probes that connect and leave at once (as the app does to find a running service) must not
  // stop the service.
  for (let i = 0; i < 50; i++) {
    await new Promise((resolve) => {
      const probe = net.createConnection(client.endpoint, () => probe.destroy());
      probe.on('close', resolve);
      probe.on('error', resolve);
    });
  }
  assert.equal((await client.call('__status')).idle, true, 'the service survives abrupt probes');
  await assert.rejects(
    client.call('list_accounts', {}, { build: 'wrong-build' }),
    /different build/,
  );
  await assert.rejects(client.call('list_accounts', {}, { protocol: 999 }), /different build/);
  await assert.rejects(client.call('create_session', {}), /workspace/);
  await assert.rejects(client.call('not_a_command'), /Unknown/);
  await assert.rejects(
    client.call('create_session', { workspace }, { command_id: null }),
    /Missing command ID/,
  );
  await assert.rejects(
    client.call('create_session', { workspace }, { command_id: 'bad' }),
    /Invalid command ID/,
  );
  const command_id = randomUUID();
  // Lose the reply after transmission, then recover the accepted result using the same ID.
  const lost = await client.open('create_session', { workspace }, { command_id });
  for (
    let n = 0;
    n < 100 && !(await client.call('list_sessions', { search: '', offset: 0 })).length;
    n++
  )
    await sleep(20);
  assert.equal((await client.call('list_sessions', { search: '', offset: 0 })).length, 1);
  lost.io.destroy();
  const replays = await Promise.all(
    Array.from({ length: 8 }, () => client.call('create_session', { workspace }, { command_id })),
  );
  assert.equal(new Set(replays.map((s) => s.id)).size, 1);
  await assert.rejects(
    client.call('create_session', { workspace: data }, { command_id }),
    /different arguments/,
  );
  const replayed = replays[0];
  assert.equal((await client.call('list_sessions', { search: '', offset: 0 })).length, 1);
  const duplicate = client.start();
  await new Promise((resolve) => duplicate.once('exit', resolve));
  assert.notEqual(duplicate.exitCode, 0);
  const sessions = await Promise.all(
    Array.from({ length: 16 }, () => client.call('create_session', { workspace })),
  );
  assert.equal(new Set(sessions.map((s) => s.id)).size, 16);
  assert.equal((await client.call('list_sessions', { search: '', offset: 0 })).length, 17);
  stream = await client.open('__events');
  assert.equal((await stream.next()).result.Ok.ready, true);
  await assert.rejects(client.call('__prepare_update'), /Close every/);
  stream.io.destroy();
  stream = undefined;
  for (let i = 0; i < 40 && (await client.call('__status')).interfaces; i++) await sleep(100);
  assert.equal((await client.call('__prepare_update')).stopped, true);
  await new Promise((resolve) =>
    daemon.exitCode !== null ? resolve() : daemon.once('exit', resolve),
  );
  let restarted = client.start();
  await client.ready();
  assert.deepEqual(await client.call('create_session', { workspace }, { command_id }), replayed);
  assert.equal((await client.call('list_sessions', { search: '', offset: 0 })).length, 17);
  if (process.platform === 'win32') {
    const codex = await client.call('add_account', { name: 'Offline Codex', provider: 'gpt' });
    const quotaHandoff = await client.call('add_account', { name: 'Handoff Claude', provider: 'claude' });
    for (const [account, model, scenario] of [
      [quotaHandoff, 'sonnet', 'CLAUDE_SUBAGENT_FIXTURE'],
      [codex, 'gpt-luna-fixture', 'CODEX_SUBAGENT_FIXTURE'],
    ]) {
      const delegated = await client.call('create_session', { workspace });
      await client.call('send_message', {
        id: delegated.id,
        text: scenario,
        options: { mode: account.provider, accountId: account.id, model, effort: 'low', accessMode: 'autopilot' },
      });
      for (let i = 0; i < 200 && !(await client.call('__status')).idle; i++) await sleep(100);
      const delegatedReply = (await client.call('get_messages', { id: delegated.id })).at(-1);
      assert.match(delegatedReply.text.trim(), /^PARENT_RESULT(?: <!-- lumen-autopilot: done -->)?$/, JSON.stringify(delegatedReply.activity.filter((a) => a.kind === 'error')));
    }
    const stoppedFolder = await mkdtemp(path.join(data, 'stop-child-'));
    const stoppedChat = await client.call('create_session', { workspace: stoppedFolder });
    const stopOptions = { mode: 'claude', accountId: quotaHandoff.id, model: 'sonnet', effort: 'low', accessMode: 'autopilot', autoReview: false };
    await client.call('send_message', { id: stoppedChat.id, text: 'DAEMON_TURN_FIXTURE', options: stopOptions });
    for (let i = 0; i < 100; i++) {
      const messages = await client.call('get_messages', { id: stoppedChat.id });
      if (messages.at(-1)?.text.includes('DAEMON_PARTIAL')) break;
      if (i === 99) assert.fail('partial reply did not reach the store');
      await sleep(50);
    }
    await client.call('cancel_run', { id: stoppedChat.id });
    for (let i = 0; i < 100 && !(await client.call('__status')).idle; i++) await sleep(50);
    const stoppedReply = (await client.call('get_messages', { id: stoppedChat.id })).at(-1);
    assert.equal(stoppedReply.text.trim(), 'DAEMON_PARTIAL');
    const stoppedSession = (await client.call('list_sessions', { search: '', offset: 0 })).find((session) => session.id === stoppedChat.id);
    assert.equal(stoppedSession.status, 'interrupted');
    const owned = JSON.parse(await readFile(path.join(stoppedFolder, 'daemon-owned-pids.json'), 'utf8'));
    for (const pid of owned) {
      assert.throws(() => process.kill(pid, 0), { code: 'ESRCH' });
    }
    await client.call('send_message', { id: stoppedChat.id, text: 'DAEMON_RESUME_FIXTURE', options: stopOptions });
    for (let i = 0; i < 100 && !(await client.call('__status')).idle; i++) await sleep(50);
    const resumed = (await client.call('get_messages', { id: stoppedChat.id })).at(-1);
    assert.match(resumed.text.trim(), /^DAEMON_RESUMED(?: <!-- lumen-autopilot: done -->)?$/);
    for (const [scenario, accessMode, status, reply, attempts] of [
      ['EDIT', 'autopilot', 'completed', 'Modifica eseguita.', ['autopilot', 'continue']],
      // An Autopilot stop the assistant explains is partial work to review, never an error.
      ['INCOMPLETE', 'autopilot', 'partial', 'Ho ancora lavoro da fare.', ['autopilot', 'continue']],
      ['UNCONFIRMED', 'autopilot', 'partial', 'Completion still unconfirmed.', ['autopilot', 'continue']],
      ['LOOP', 'autopilot', 'partial', 'Repeated verification 3.', ['autopilot', 'continue', 'continue', 'continue']],
      ['LIMIT', 'autopilot', 'partial', 'New check 9 passed.', ['autopilot', ...Array(9).fill('continue')]],
      ['BLOCKED', 'autopilot', 'partial', 'Il servizio richiesto non risponde.', []],
      ['QUESTION', 'autopilot', 'completed', 'Risposta alla domanda.', []],
      ['DECISION', 'autopilot', 'completed', 'Priority confirmed.', []],
      ['EDIT', 'read-only', 'completed', /ho individuato il file/i, ['restricted']],
      ['HANDOFF', 'autopilot', 'completed', 'Handoff completed.', ['autopilot', 'continue', 'continue', 'continue', 'continue']],
    ]) {
      const folder = await mkdtemp(path.join(data, 'autopilot-'));
      const attachments = [];
      if (scenario === 'EDIT' && accessMode === 'autopilot') {
        await writeFile(path.join(folder, 'autopilot-can-finish.txt'), '');
        const screenshot = 'autopilot-reference.png';
        const pixel = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jOIoAAAAASUVORK5CYII=';
        await writeFile(path.join(folder, screenshot), Buffer.from(pixel, 'base64'));
        attachments.push(screenshot, path.join(folder, screenshot));
      }
      const { id } = await client.call('create_session', { workspace: folder });
      const handoff = scenario === 'HANDOFF';
      await client.call('send_message', {
        id,
        text: `AUTOPILOT_FIXTURE_${scenario}`,
        options: handoff
          ? { mode: 'claude', accountId: quotaHandoff.id, model: 'sonnet', effort: 'low', accessMode }
          : { mode: 'gpt', model: 'gpt-luna-fixture', effort: 'low', accessMode, attachments },
      });
      if (handoff || scenario === 'DECISION') {
        let question;
        for (let i = 0; i < 200 && !question; i++) {
          question = (await client.call('pending_questions')).at(-1);
          if (!question) await sleep(100);
        }
        assert(question, 'Autopilot waits for the structured answer');
        await client.call('answer_question', {
          id: question,
          answers: handoff ? { switch: ['Continue'] } : { priority: ['Quality'] },
        });
      }
      for (let i = 0; i < 200 && !(await client.call('__status')).idle; i++) await sleep(100);
      const session = (await client.call('list_sessions', { search: '', offset: 0 })).find(
        (s) => s.id === id,
      );
      assert.equal(session?.status, status, `${scenario} ${accessMode}`);
      const response = (await client.call('get_messages', { id })).at(-1);
      const stop = { INCOMPLETE: 'stalled', UNCONFIRMED: 'stalled', LOOP: 'stalled', LIMIT: 'limit', BLOCKED: 'blocked' }[scenario];
      const stops = response.activity.filter((a) => a.kind === 'autopilot-stopped').map((a) => JSON.parse(a.text).reason);
      assert.deepEqual(stops, stop ? [stop] : [], `${scenario} stop reason`);
      if (stop) assert(!response.activity.some((a) => a.kind === 'error'), `${scenario} is not an error`);
      if (typeof reply === 'string') assert.equal(response.text, reply);
      else assert.match(response.text, reply);
      const attemptsFile = path.join(folder, 'autopilot-attempts.txt');
      if (attempts.length) {
        assert.deepEqual((await readFile(attemptsFile, 'utf8')).trim().split(/\r?\n/), attempts);
      } else {
        await assert.rejects(readFile(attemptsFile), { code: 'ENOENT' });
      }
      const result = path.join(folder, 'autopilot-result.txt');
      if (handoff || (scenario === 'EDIT' && accessMode === 'autopilot')) {
        assert.equal((await readFile(result, 'utf8')).trim(), 'written once');
      } else {
        await assert.rejects(readFile(result), { code: 'ENOENT' });
      }
      if (handoff) {
        for (let step = 1; step < 4; step++) {
          assert.equal((await readFile(path.join(folder, `autopilot-handoff-step-${step}.txt`), 'utf8')).trim(), `step ${step}`);
        }
      }
    }
    const spend = await client.call('create_session', { workspace });
    await client.call('send_message', { id: spend.id, text: 'CLAUDE_SPEND_FIXTURE', options: { ...stopOptions, model: 'opus' } });
    for (let i = 0; i < 100 && !(await client.call('__status')).idle; i++) await sleep(100);
    const spendReply = (await client.call('get_messages', { id: spend.id })).at(-1);
    assert.match(JSON.stringify(spendReply.activity), /could not verify an account quota limit/);
    assert.ok(!spendReply.activity.some((step) => step.kind === 'quota-reset'));
    const afterSpend = await client.call('list_models', { id: quotaHandoff.id, refresh: true });
    assert.ok(!afterSpend.some((model) => model.id === 'opus'));
    assert.ok(!afterSpend.some((model) => model.id === 'claude-opus-5-5'));
    assert.ok(afterSpend.some((model) => model.id === 'sonnet'));
    await client.call('remove_account', { id: quotaHandoff.id });
    const claude = await client.call('add_account', { name: 'Offline Claude', provider: 'claude' });
    const verification = await mkdtemp(path.join(data, 'verify-'));
    await mkdir(path.join(verification, '.lumen'));
    execFileSync('git', ['init', '-q'], { cwd: verification, windowsHide: true });
    await writeFile(path.join(verification, '.lumen', 'diagnostics.json'), '{"servers":[]}');
    await writeFile(path.join(verification, '.lumen', 'checks.json'), JSON.stringify([
      { name: 'Verification probe', command: 'node verification-probe.cjs', automatic: true },
    ]));
    await writeFile(path.join(verification, 'verification-probe.cjs'), `
      const broken = require('node:fs').readFileSync('verification-target.txt', 'utf8').includes('broken');
      if (broken) console.log('error: EVIDENCE_FIXTURE_ERROR');
      console.log('Compiling EVIDENCE_FIXTURE_NOISE\\n'.repeat(3000));
      process.exitCode = broken ? 3 : 0;
    `);
    const settings = await client.call('get_settings');
    await client.call('save_settings', { value: { ...settings, verifyChanges: true } });
    const verificationChat = await client.call('create_session', { workspace: verification });
    await client.call('send_message', {
      id: verificationChat.id,
      text: 'VERIFY_AND_REVIEW_FIXTURE',
      options: { mode: 'claude', accountId: claude.id, model: 'sonnet', effort: 'low', accessMode: 'autopilot', autoReview: true },
    });
    for (let i = 0; i < 200 && !(await client.call('__status')).idle; i++) await sleep(100);
    const verified = (await client.call('get_messages', { id: verificationChat.id })).at(-1);
    const activities = verified.activity.filter((step) => ['validation', 'review'].includes(step.kind));
    assert.deepEqual(activities.map((step) => step.kind === 'validation' ? JSON.parse(step.text).status : step.kind), ['failed', 'passed', 'review']);
    assert.match(activities.at(-1).text, /APPROVED/);
    assert.match(activities[0].text, /EVIDENCE_FIXTURE_ERROR/);
    assert.doesNotMatch(activities[0].text, /EVIDENCE_FIXTURE_NOISE/);
    const captured = verified.activity.find((step) => step.kind === 'verify' && step.detail);
    assert.ok(captured, 'large captured check output should be expandable');
    const capturedText = await client.call('activity_detail', { id: verificationChat.id, message: verified.id, digest: captured.detail.digest });
    assert.match(capturedText, /EVIDENCE_FIXTURE_NOISE/);
    assert.match(capturedText, /Middle of check output omitted at the capture limit/);
    assert.ok(JSON.parse(activities[0].text).detail.length < capturedText.length / 100);
    await client.call('save_settings', { value: settings });
    await client.call('send_message', {
      id: verificationChat.id,
      text: 'VERIFY_HANDOFF_FIXTURE',
      options: { mode: 'gpt', accountId: codex.id, model: 'gpt-luna-fixture', effort: 'low', accessMode: 'read-only' },
    });
    for (let i = 0; i < 200 && !(await client.call('__status')).idle; i++) await sleep(100);
    assert.equal((await client.call('get_messages', { id: verificationChat.id })).at(-1).text, 'VERIFICATION_HANDOFF_OK');
    const folder = await mkdtemp(path.join(data, 'router-'));
    const { id } = await client.call('create_session', { workspace: folder });
    const plan = await client.call('send_message', {
      id,
      text: 'Investigate rare divergence across systems with validation of ordering invariants. AUTOPILOT_FIXTURE_QUESTION',
      options: { mode: 'auto', effort: '', accessMode: 'autopilot' },
    });
    assert.equal(plan.model, 'gpt-astra-fixture');
    assert.equal(plan.effort, 'max');
    for (let i = 0; i < 200 && !(await client.call('__status')).idle; i++) await sleep(100);
    const result = (await client.call('get_messages', { id })).at(-1);
    assert.equal(result.text, 'Risposta alla domanda.');
    for (const [autoPolicy, expectedAccount, used] of [
      ['budget', codex.id, [15, 85]],
      ['default', claude.id, [15, 85]],
      ['quality', codex.id, [85, 15]],
    ]) {
      const chat = await client.call('create_session', { workspace: folder });
      await client.call('send_message', {
        id: chat.id,
        text: 'Read the previous project context.',
        options: { mode: 'claude', effort: '', accessMode: 'read-only' },
      });
      for (let i = 0; i < 200 && !(await client.call('__status')).idle; i++) await sleep(100);
      const now = Date.now() - 100;
      await writeFile(path.join(data, 'usage.json'), JSON.stringify(
        [codex, claude].map((account, i) => ({
          accountId: account.id,
          updatedAt: now,
          source: 'fixture',
          windows: [{ usedPercent: used[i], windowMinutes: 300, resetsAt: now + 300 * 60_000 }],
        })),
      ));
      const routed = await client.call('send_message', {
        id: chat.id,
        text: 'Da uno screenshot delle impostazioni, guidami nel trovare la sezione delle notifiche.',
        options: { mode: 'auto', effort: '', accessMode: 'read-only', autoPolicy },
      });
      assert.equal(routed.accountId, expectedAccount, autoPolicy);
      for (let i = 0; i < 200 && !(await client.call('__status')).idle; i++) await sleep(100);
    }
    const origin = await mkdtemp(path.join(data, 'auto-quota-'));
    const project = await client.call('add_workspace', { name: 'Quota project', folder: origin });
    const git = (cwd, ...args) => execFileSync('git', args, { cwd, windowsHide: true, encoding: 'utf8' }).trim();
    git(origin, 'init', '-q', '-b', 'main');
    await writeFile(path.join(origin, 'auto-quota-project.txt'), 'Project marker');
    git(origin, 'add', 'auto-quota-project.txt');
    git(origin, '-c', 'user.name=Lumen Fixture', '-c', 'user.email=lumen@example.invalid', 'commit', '-qm', 'Fixture project');
    const chat = await client.call('create_session', { workspace: origin });
    const session = async () => (await client.call('list_sessions', { search: '', offset: 0 })).find((s) => s.id === chat.id);
    const waitForStatus = async (expected) => {
      for (let i = 0; i < 240; i++) {
        const current = await session();
        if (current?.status === expected) return current;
        await sleep(100);
      }
      assert.equal((await session())?.status, expected);
    };
    await client.call('set_session_access_mode', { id: chat.id, mode: 'autopilot' });
    const routed = await client.call('send_message', {
      id: chat.id,
      text: 'Investigate rare divergence across systems with validation of ordering invariants. Implement the fix. AUTO_QUOTA_RESTART_FIXTURE',
      options: { mode: 'auto', effort: '', accessMode: 'autopilot' },
    });
    assert.equal(routed.accountId, codex.id);
    assert.equal(routed.model, 'gpt-astra-fixture');
    assert.equal(routed.effort, 'max');
    const interrupted = await waitForStatus('error');
    const task = await client.call('task_worktree_status', { id: chat.id, preview: false });
    assert.equal(task?.origin, project.folder);
    assert.equal(interrupted.workspaceId, project.id);
    assert.notEqual(interrupted.workspace, project.folder);
    const previous = (await client.call('get_messages', { id: chat.id })).at(-1);
    assert.match(previous.text, /Recorded the first edit/);
    const retry = JSON.parse(previous.activity.find((step) => step.kind === 'quota-reset').text);
    assert.equal(retry.accountId, codex.id);
    assert.equal(retry.model, routed.model);
    assert.equal(retry.effort, routed.effort);
    await client.call('resume_at_reset', { id: chat.id, message: previous.id });
    await waitForStatus('waiting');
    const oldProcess = restarted;
    const exited = new Promise((resolve) => oldProcess.once('exit', resolve));
    oldProcess.kill('SIGKILL');
    await exited;
    restarted = client.start();
    await client.ready();
    const restored = await waitForStatus('waiting');
    assert.ok(Date.now() < retry.at, 'restart must occur before the simulated reset');
    assert.equal(restored.workspace, interrupted.workspace);
    assert.equal(restored.workspaceId, project.id);
    const health = await client.call('provider_health');
    assert.equal(health.find((entry) => entry.accountId === codex.id)?.state, 'cooldown');
    const read = async (folder, name) => (await readFile(path.join(folder, name), 'utf8')).trim();
    assert.equal(await read(restored.workspace, 'quota-attempts.txt'), 'initial');
    const finished = await waitForStatus('completed');
    assert.equal(finished.workspace, restored.workspace);
    assert.equal(finished.workspaceId, project.id);
    checkQuotaReplay = async () => {
      const history = await client.call('get_messages', { id: chat.id });
      assert.equal(history.filter((message) => message.role === 'user').length, 2);
      assert.match(history.find((message) => message.id === previous.id).text, /Recorded the first edit/);
      assert.equal(history.at(-1).text, 'Finished after reset.');
      assert.equal(await read(finished.workspace, 'quota-effect.txt'), 'once');
      assert.deepEqual((await read(finished.workspace, 'quota-attempts.txt')).split(/\r?\n/), ['initial', 'resume']);
    };
    await checkQuotaReplay();
    assert.equal(await read(finished.workspace, 'quota-finished.txt'), 'finished');
    await assert.rejects(read(project.folder, 'quota-effect.txt'), { code: 'ENOENT' });
    await assert.rejects(read(workspace, 'quota-effect.txt'), { code: 'ENOENT' });

    const secondCodex = await client.call('add_account', { name: 'Second Offline Codex', provider: 'gpt' });
    const secondClaude = await client.call('add_account', { name: 'Second Offline Claude', provider: 'claude' });
    const research = 'Research primary sources and synthesize the trade-offs between routing approaches.';
    const architecture = 'Research primary sources and synthesize the trade-offs between architecture approaches.';
    const week = 3 * 24 * 60;
    // Entries give the remaining quota at five hours and one week, then minutes to weekly reset.
    for (const [name, autoPolicy, prompt, quotas, expected, model, effort] of [
      ['more quota', 'default', research, [[5, 60], [80, 60], [1, 1]], secondCodex, 'gpt-fixture', 'high'],
      ['earlier weekly reset', 'quality', research, [[20, 20], [20, 20, 20], [1, 1]], secondCodex, 'gpt-fixture', 'high'],
      ['more weekly quota', 'budget', research, [[5, 5], [5, 80], [1, 1]], secondCodex, 'gpt-fixture', 'medium'],
      ['quality keeps provider', 'quality', architecture, [[100, 100], [80, 80], [5, 5]], claude, 'opus', 'high'],
      ['budget switches provider', 'budget', architecture, [[100, 100], [80, 80], [5, 5]], codex, 'gpt-fixture', 'medium'],
      ['explicit provider in Budget', 'budget', `Use Claude: ${architecture}`, [[100, 100], [80, 80], [5, 5]], claude, 'opus', 'medium'],
      ['balance two Claude accounts', 'quality', architecture, [[100, 100], [80, 80], [5, 5], [80, 80]], secondClaude, 'opus', 'high'],
      ['equal quota keeps the original account', 'default', research, [[20, 20], [20, 20], [1, 1]], codex, 'gpt-fixture', 'high'],
    ]) {
      const now = Date.now() - 100;
      await writeFile(path.join(data, 'usage.json'), JSON.stringify(
        [codex, secondCodex, claude, secondClaude].map((account, i) => {
          const [short, weekly, weeklyReset = week] = quotas[i] ?? [1, 1];
          return {
            accountId: account.id,
            updatedAt: now,
            source: 'fixture',
            windows: [
              { usedPercent: 100 - short, windowMinutes: 300, resetsAt: now + 300 * 60_000 },
              { usedPercent: 100 - weekly, windowMinutes: 10080, resetsAt: now + weeklyReset * 60_000 },
            ],
          };
        }),
      ));
      const { id } = await client.call('create_session', { workspace: folder });
      const route = await client.call('send_message', {
        id, text: prompt, options: { mode: 'auto', effort: '', accessMode: 'read-only', autoPolicy },
      });
      assert.deepEqual([route.accountId, route.model, route.effort], [expected.id, model, effort], name);
      for (let i = 0; i < 200 && !(await client.call('__status')).idle; i++) await sleep(100);
    }

    // D1: when the only available model is weaker than the task and the preferred account
    // is out of quota, the downgrade needs consent. Autopilot asks interactively: the
    // question names the blocked provider and its reset, and Continue finishes the turn.
    const thirdCodex = await client.call('add_account', { name: 'Third Offline Codex', provider: 'gpt' });
    await writeFile(path.join(data, 'accounts', thirdCodex.id, 'small-catalog-fixture'), '');
    const exhaustedAt = Date.now() - 100;
    await writeFile(path.join(data, 'usage.json'), JSON.stringify(
      [codex, secondCodex, thirdCodex, claude, secondClaude].map((account, i) => ({
        accountId: account.id,
        updatedAt: exhaustedAt,
        source: 'fixture',
        windows: i === 2 ? [
          { usedPercent: 20, windowMinutes: 300, resetsAt: exhaustedAt + 300 * 60_000 },
          { usedPercent: 20, windowMinutes: 10080, resetsAt: exhaustedAt + week * 60_000 },
        ] : [
          { usedPercent: 100, windowMinutes: 300, resetsAt: exhaustedAt + 120 * 60_000 },
          { usedPercent: 100, windowMinutes: 10080, resetsAt: exhaustedAt + 48 * 60 * 60_000 },
        ],
      })),
    ));
    const consent = await client.call('create_session', { workspace: folder });
    const downgraded = await client.call('send_message', {
      id: consent.id,
      text: architecture,
      options: { mode: 'auto', effort: '', accessMode: 'autopilot' },
    });
    assert.deepEqual([downgraded.accountId, downgraded.model], [thirdCodex.id, 'gpt-terra-fixture']);
    let waiting;
    for (let i = 0; i < 200 && !waiting; i++) {
      waiting = (await client.call('pending_questions')).at(-1);
      if (!waiting) await sleep(100);
    }
    assert(waiting, 'the downgrade asks before anything is sent');
    const asked = (await client.call('get_messages', { id: consent.id }))
      .flatMap((message) => message.activity)
      .find((step) => step.kind === 'interaction');
    const question = JSON.parse(asked.text).questions[0].question;
    assert.match(question, /No available model meets this task's retained difficulty\./, question);
    assert.match(question, /The Claude account has a suitable model, but its quota is exhausted and resets in about 2 hours\./, question);
    await client.call('answer_question', { id: waiting, answers: { switch: ['Continue'] } });
    for (let i = 0; i < 200 && !(await client.call('__status')).idle; i++) await sleep(100);
    assert((await client.call('__status')).idle, 'the turn finished on the weaker model after consent');

    // OpenCode Go is the emergency chain after Claude and ChatGPT: with every primary
    // account out of quota, Auto runs the curated tier model; a pinned model wins always.
    const opencode = await client.call('add_account', { name: 'Offline OpenCode', provider: 'opencode' });
    await writeFile(path.join(data, 'usage.json'), JSON.stringify(
      [codex, secondCodex, thirdCodex, claude, secondClaude].map((account) => {
        const at = Date.now() - 100;
        return {
          accountId: account.id,
          updatedAt: at,
          source: 'fixture',
          windows: [
            { usedPercent: 100, windowMinutes: 300, resetsAt: at + 120 * 60_000 },
            { usedPercent: 100, windowMinutes: 10080, resetsAt: at + 48 * 60 * 60_000 },
          ],
        };
      }),
    ));
    const emergency = await client.call('create_session', { workspace: folder });
    const fallback = await client.call('send_message', {
      id: emergency.id,
      text: architecture,
      options: { mode: 'auto', effort: '', accessMode: 'read-only' },
    });
    assert.deepEqual(
      [fallback.accountId, fallback.model],
      [opencode.id, 'opencode-go/qwen4-max'],
      'the tier-two emergency model runs when every primary is exhausted',
    );
    for (let i = 0; i < 200 && !(await client.call('__status')).idle; i++) await sleep(100);
    const emergencyReply = (await client.call('get_messages', { id: emergency.id })).at(-1);
    assert.match(emergencyReply.text, /OPENCODE_FIRST_OK/);
    const appSettings = await client.call('get_settings');
    await client.call('save_settings', {
      value: { ...appSettings, emergencyModels: { opencode: 'opencode-go/deepseek-v4.1-flash' } },
    });
    const pinned = await client.call('create_session', { workspace: folder });
    const pinnedRoute = await client.call('send_message', {
      id: pinned.id,
      text: architecture,
      options: { mode: 'auto', effort: '', accessMode: 'read-only' },
    });
    assert.deepEqual(
      [pinnedRoute.accountId, pinnedRoute.model],
      [opencode.id, 'opencode-go/deepseek-v4.1-flash'],
      'a pinned emergency model is the only one used',
    );
    for (let i = 0; i < 200 && !(await client.call('__status')).idle; i++) await sleep(100);
    // Fresh quota alone must not prevent fallback when all primaries are confirmed signed out.
    await writeFile(path.join(data, 'usage.json'), '[]');
    for (const account of [codex, secondCodex, thirdCodex, claude, secondClaude]) {
      await writeFile(path.join(data, 'accounts', account.id, 'signed-out-fixture'), '');
      // Auto remembers sign-in checks for a few minutes; a status check from Accounts refreshes them.
      await client.call('account_status', { id: account.id });
    }
    const signedOutSession = await client.call('create_session', { workspace: folder });
    const signedOutRoute = await client.call('send_message', {
      id: signedOutSession.id,
      text: architecture,
      options: { mode: 'auto', effort: '', accessMode: 'read-only' },
    });
    assert.equal(signedOutRoute.accountId, opencode.id, 'confirmed logout triggers emergency fallback');
    for (let i = 0; i < 200 && !(await client.call('__status')).idle; i++) await sleep(100);

    // Stop while Auto is still preparing (here a slow sign-in check) cancels the turn: no message
    // is recorded and nothing starts afterwards.
    await rm(path.join(data, 'accounts', claude.id, 'signed-out-fixture'));
    await writeFile(path.join(data, 'accounts', claude.id, 'slow-status-fixture'), '');
    await client.call('account_status', { id: claude.id });
    const preparing = await client.call('create_session', { workspace: folder });
    const pending = client
      .call('send_message', {
        id: preparing.id,
        text: architecture,
        options: { mode: 'auto', effort: '', accessMode: 'read-only' },
      })
      .then(
        () => 'started',
        (error) => String(error),
      );
    try {
      // The preparing turn holds a job: the service stops reporting idle until it ends.
      for (let i = 0; i < 100 && (await client.call('__status')).idle; i++) await sleep(20);
      assert(!(await client.call('__status')).idle, 'the turn is preparing');
      await client.call('cancel_run', { id: preparing.id });
      assert.match(await pending, /Cancelled/, 'Stop during preparation cancels the turn');
      assert.equal((await client.call('get_messages', { id: preparing.id })).length, 0, 'no message is recorded');
      assert((await client.call('__status')).idle, 'nothing runs after a cancelled preparation');
    } finally {
      await rm(path.join(data, 'accounts', claude.id, 'slow-status-fixture'), { force: true });
    }
  }
  await writeFile(
    path.join(workspace, 'package.json'),
    JSON.stringify({ scripts: { test: 'node owned-child.mjs' } }),
  );
  await writeFile(
    path.join(workspace, 'owned-child.mjs'),
    `import {spawn} from 'node:child_process';import{writeFileSync}from'node:fs';const child=spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'ignore'});writeFileSync('owned-pids.json',JSON.stringify([process.pid,child.pid]));setInterval(()=>{},1000);`,
  );
  const check = (await client.call('project_checks', { folder: workspace })).find(
    (c) => c.id === 'npm:.:test',
  );
  assert(check, 'owned package test is discovered');
  let commandError;
  const running = client
    .call('run_project_check', {
      folder: workspace,
      id: check.id,
      command: check.command,
      cwd: check.cwd,
    })
    .catch((e) => {
      commandError = e;
    });
  let owned;
  for (let i = 0; i < 100; i++) {
    try {
      owned = JSON.parse(await readFile(path.join(workspace, 'owned-pids.json'), 'utf8'));
      break;
    } catch {
      await sleep(100);
    }
  }
  assert(owned, `owned child started: ${commandError || 'no child identities'}`);
  const unrelated = spawn(process.execPath, ['-e', 'setInterval(()=>{},1000)'], {
    stdio: 'ignore',
    windowsHide: true,
  });
  const alive = (pid) => {
    try {
      process.kill(pid, 0);
      if (
        process.platform !== 'win32' &&
        execFileSync('ps', ['-o', 'stat=', '-p', String(pid)], { encoding: 'utf8' })
          .trim()
          .startsWith('Z')
      )
        return false;
      return true;
    } catch {
      return false;
    }
  };
  try {
    restarted.kill('SIGKILL');
    await running;
    for (let i = 0; i < 100 && owned.some(alive); i++) await sleep(50);
    assert(
      owned.every((p) => !alive(p)),
      'service crash must clean owned descendants',
    );
    assert(alive(unrelated.pid), 'unrelated process is preserved');
  } finally {
    unrelated.kill();
  }
  const recovered = client.start();
  await client.ready();
  if (checkQuotaReplay) {
    assert.equal((await client.call('__status')).idle, true);
    await checkQuotaReplay();
  }
  await client.call('__shutdown');
  await new Promise((resolve) =>
    recovered.exitCode !== null ? resolve() : recovered.once('exit', resolve),
  );
  await new Promise((resolve) =>
    restarted.exitCode !== null || restarted.signalCode !== null
      ? resolve()
      : restarted.once('exit', resolve),
  );
  console.log(
    JSON.stringify(
      {
        passed: true,
        checks: [
          'headless startup',
          'build/protocol authentication',
          'typed argument boundary',
          'single owner',
          'abrupt connection probes',
          'concurrent commands',
          'lost reply and concurrent duplicate command replay without repeated effects',
          'command payload binding and receipt persistence across restart',
          'event subscription',
          'connected update refusal',
          'drain and restart preserving data',
          ...(process.platform === 'win32' ? [
            'Codex Autopilot resumes while work advances, stops repeated checks or after 10 attempts as partial work, and waits for answers',
            'Claude and Codex deny child requests and wait for the parent result',
            'Stop terminates owned children and a follow-up preserves the existing effect',
            'Compact verification evidence survives repair, independent review and account handoff',
            'Auto selects a sufficient model from another catalog',
            'Auto resumes the same model, effort and worktree after quota and a daemon crash without replay',
            'Auto falls back to the OpenCode Go emergency model only when every Claude and ChatGPT account is out of quota',
          ] : []),
          'hard crash cleans owned descendants and preserves unrelated processes',
        ],
      },
      null,
      2,
    ),
  );
} finally {
  stream?.io.destroy();
  await client.call('__shutdown').catch(() => {});
  if (daemon.exitCode === null) daemon.kill();
}
