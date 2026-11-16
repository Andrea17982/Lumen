import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile } from 'node:fs/promises';
import { spawn, execFileSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import path from 'node:path';
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
  const duplicate = client.start();
  await new Promise((resolve) => duplicate.once('exit', resolve));
  assert.notEqual(duplicate.exitCode, 0);
  const sessions = await Promise.all(
    Array.from({ length: 16 }, () => client.call('create_session', { workspace })),
  );
  assert.equal(new Set(sessions.map((s) => s.id)).size, 16);
  assert.equal((await client.call('list_sessions', { search: '', offset: 0 })).length, 16);
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
  assert.equal((await client.call('list_sessions', { search: '', offset: 0 })).length, 16);
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
          'event subscription',
          'connected update refusal',
          'drain and restart preserving data',
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
