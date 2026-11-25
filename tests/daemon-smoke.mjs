import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile } from 'node:fs/promises';
import { spawn, execFileSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
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
try {
  assert.equal((await client.ready()).idle, true, log);
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
  const restarted = client.start();
  await client.ready();
  assert.deepEqual(await client.call('create_session', { workspace }, { command_id }), replayed);
  assert.equal((await client.call('list_sessions', { search: '', offset: 0 })).length, 17);
  if (process.platform === 'win32') {
    const codex = await client.call('add_account', { name: 'Offline Codex', provider: 'gpt' });
    for (const [scenario, accessMode, status, reply, attempts] of [
      ['EDIT', 'autopilot', 'completed', 'Modifica eseguita.', ['autopilot', 'continue']],
      ['INCOMPLETE', 'autopilot', 'error', 'Ho ancora lavoro da fare.', ['autopilot', 'continue']],
      ['BLOCKED', 'autopilot', 'error', 'Il servizio richiesto non risponde.', []],
      ['QUESTION', 'autopilot', 'completed', 'Risposta alla domanda.', []],
      ['EDIT', 'read-only', 'completed', /ho individuato il file/i, ['restricted']],
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
      await client.call('send_message', {
        id,
        text: `AUTOPILOT_FIXTURE_${scenario}`,
        options: { mode: 'gpt', model: 'gpt-luna-fixture', effort: 'low', accessMode, attachments },
      });
      for (let i = 0; i < 200 && !(await client.call('__status')).idle; i++) await sleep(100);
      const session = (await client.call('list_sessions', { search: '', offset: 0 })).find(
        (s) => s.id === id,
      );
      assert.equal(session?.status, status, `${scenario} ${accessMode}`);
      const response = (await client.call('get_messages', { id })).at(-1);
      if (typeof reply === 'string') assert.equal(response.text, reply);
      else assert.match(response.text, reply);
      const attemptsFile = path.join(folder, 'autopilot-attempts.txt');
      if (attempts.length) {
        assert.deepEqual((await readFile(attemptsFile, 'utf8')).trim().split(/\r?\n/), attempts);
      } else {
        await assert.rejects(readFile(attemptsFile), { code: 'ENOENT' });
      }
      const result = path.join(folder, 'autopilot-result.txt');
      if (scenario === 'EDIT' && accessMode === 'autopilot') {
        assert.equal((await readFile(result, 'utf8')).trim(), 'written once');
      } else {
        await assert.rejects(readFile(result), { code: 'ENOENT' });
      }
    }
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
          'concurrent commands',
          'lost reply and concurrent duplicate command replay without repeated effects',
          'command payload binding and receipt persistence across restart',
          'event subscription',
          'connected update refusal',
          'drain and restart preserving data',
          ...(process.platform === 'win32' ? [
            'Codex Autopilot completion and bounded native continuation',
            'Compact verification evidence survives repair, independent review and account handoff',
            'Auto selects a sufficient model from another catalog',
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
