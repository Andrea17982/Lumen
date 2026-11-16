// Real GUI/service lifetimes, SQLite and owned process trees; offline CLI protocols only.
import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { nativeHarness } from './native-harness.mjs';
import { daemonClient, sleep } from './daemon-harness.mjs';

let h = await nativeHarness('daemon-lifetime');
const data = h.data,
  workspace = h.workspace,
  checks = [];
const client = await daemonClient(data);
const initialPid = (await client.call('__status')).pid;
const account = await h.invoke('add_account', {
  name: 'Service lifetime fixture',
  provider: 'claude',
});
const options = {
  mode: 'claude',
  accountId: account.id,
  model: 'sonnet',
  effort: 'low',
  accessMode: 'autopilot',
  autoReview: false,
};
async function until(test, label) {
  for (let i = 0; i < 400; i++) {
    const value = await test();
    if (value) return value;
    await sleep(100);
  }
  throw new Error(`Timed out: ${label}`);
}
const messages = (id) => h.invoke('get_messages', { id });
const status = async (id) =>
  (await h.invoke('list_sessions', { search: '', offset: 0 })).find((s) => s.id === id)?.status;
async function reopen() {
  await h.close({ keepDaemon: true });
  h = await nativeHarness('daemon-lifetime', { dataDirectory: data });
}
const alive = (pid) => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};
let unrelated;
try {
  const chat = await h.invoke('create_session', { workspace });
  await h.invoke('send_message', { id: chat.id, text: 'DAEMON_TURN_FIXTURE', options });
  await until(
    async () => (await messages(chat.id)).at(-1)?.text.includes('DAEMON_PARTIAL'),
    'partial output',
  );
  const initialPrefs = await h.invoke('get_settings');
  await h.invoke('save_settings', { value: { ...initialPrefs, closeToTray: false } });
  await h.invoke('plugin:window|close', { label: 'main' }).catch(() => {});
  await until(async () => !alive(h.pid), 'normal interface close');
  await reopen();
  assert.equal((await client.call('__status')).pid, initialPid);
  assert((await messages(chat.id)).at(-1).text.includes('DAEMON_PARTIAL'));
  await until(
    async () => (await status(chat.id)) === 'completed',
    'turn completion after GUI exit',
  );
  assert.equal(
    (await readFile(path.join(workspace, 'daemon-effect.txt'), 'utf8')).trim().split(/\r?\n/)
      .length,
    1,
  );
  checks.push(
    'GUI exit/reopen preserves service PID, partial output and exactly one owned turn effect',
  );

  const question = await h.invoke('create_session', { workspace });
  await h.invoke('send_message', { id: question.id, text: 'QUESTION_FIXTURE claude', options });
  const request = await until(async () => {
    const row = (await messages(question.id))
      .flatMap((m) => m.activity)
      .find((a) => a.kind === 'interaction');
    return row && JSON.parse(row.text);
  }, 'native question');
  await reopen();
  assert((await h.invoke('pending_questions')).includes(request.id));
  await h.invoke('answer_question', {
    id: request.id,
    answers: { [request.questions[0].id]: ['Native'] },
  });
  await until(
    async () => (await status(question.id)) === 'completed',
    'answer reaches the original CLI',
  );
  assert.equal(
    (await readFile(path.join(workspace, 'answer-claude.txt'), 'utf8')).trim(),
    'Native',
  );
  assert(!(await h.invoke('pending_questions')).includes(request.id));
  checks.push('Pending native question survives full GUI restart and answers the same CLI once');

  const completed = (await messages(question.id)).at(-1);
  const at = Date.now() + 120000;
  const activity = [
    ...completed.activity,
    { kind: 'error', text: 'Owned quota scheduling fixture' },
    {
      kind: 'quota-reset',
      text: JSON.stringify({ at, accountId: account.id, model: 'sonnet', effort: 'low' }),
    },
  ];
  execFileSync(
    'python',
    [
      '-c',
      'import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); c.execute("UPDATE messages SET activity=? WHERE id=?",(sys.argv[2],sys.argv[3])); c.commit()',
      path.join(data, 'lumen.db'),
      JSON.stringify(activity),
      completed.id,
    ],
    { windowsHide: true },
  );
  await h.invoke('resume_at_reset', { id: question.id, message: completed.id });
  assert.equal(await status(question.id), 'waiting');
  await h.close();
  h = await nativeHarness('daemon-lifetime', { dataDirectory: data });
  await until(
    async () => (await status(question.id)) === 'waiting',
    'durable quota timer restored',
  );
  await h.invoke('cancel_run', { id: question.id });
  await until(
    async () => (await status(question.id)) === 'interrupted',
    'quota timer cancellation',
  );
  const count = execFileSync(
    'python',
    [
      '-c',
      'import sqlite3,sys; print(sqlite3.connect(sys.argv[1]).execute("SELECT count(*) FROM quota_schedules").fetchone()[0])',
      path.join(data, 'lumen.db'),
    ],
    { windowsHide: true, encoding: 'utf8' },
  ).trim();
  assert.equal(count, '0');
  checks.push(
    'Quota intention survives orderly service restart without inference and remains cancellable',
  );

  const git = (...args) =>
    execFileSync('git', ['-C', workspace, ...args], { windowsHide: true, encoding: 'utf8' }).trim();
  git('init');
  git(
    '-c',
    'user.name=Fixture',
    '-c',
    'user.email=fixture@example.invalid',
    'commit',
    '--allow-empty',
    '-m',
    'Owned service fixture',
  );
  const prefs = await h.invoke('get_settings');
  await h.invoke('save_settings', { value: { ...prefs, orchestra: true } });
  const orchestra = await h.invoke('create_session', { workspace });
  const orchOptions = {
    ...options,
    accessMode: 'file-edits',
    mode: 'auto',
    accountId: null,
    model: null,
    effort: '',
  };
  await h.invoke('send_message', {
    id: orchestra.id,
    text: 'Implement the authentication architecture for the API and settings UI. DAEMON_ORCHESTRA_FIXTURE',
    options: orchOptions,
  });
  await until(async () => (await status(orchestra.id)) === 'completed', 'Orchestra questions');
  const card = async () => {
    const m = (await messages(orchestra.id)).findLast((row) =>
      row.activity.some((a) => a.kind === 'orchestra'),
    );
    return [m, JSON.parse(m.activity.find((a) => a.kind === 'orchestra').text)];
  };
  let [m, state] = await card();
  assert.equal(state.stage, 'questions');
  await h.invoke('orchestra_answer', {
    id: orchestra.id,
    message: m.id,
    answers: [],
    skip: true,
    options: orchOptions,
  });
  await until(async () => (await status(orchestra.id)) === 'completed', 'Orchestra plan');
  [m, state] = await card();
  assert.equal(state.stage, 'plan');
  await h.invoke('orchestra_approve', {
    id: orchestra.id,
    message: m.id,
    edits: [],
    options: orchOptions,
  });
  await until(async () => {
    const [, s] = await card();
    return s.subtasks.some((t) => t.status === 'running');
  }, 'Orchestra worker');
  await until(async () => {
    const [m] = await card();
    return Object.values(m.live || {}).some((text) => text.includes('ORCHESTRA_WORKER_PARTIAL'));
  }, 'live worker output');
  const orchestraPid = (await client.call('__status')).pid;
  await reopen();
  assert.equal((await client.call('__status')).pid, orchestraPid);
  assert(
    Object.values((await card())[0].live || {}).some((text) =>
      text.includes('ORCHESTRA_WORKER_PARTIAL'),
    ),
    'worker partial output survives GUI reopen',
  );
  await until(
    async () => (await status(orchestra.id)) === 'completed',
    'Orchestra after GUI restart',
  );
  [, state] = await card();
  assert.equal(state.stage, 'done');
  assert(state.subtasks.every((t) => t.status === 'done'));
  assert.equal(git('worktree', 'list').split(/\r?\n/).length, 1);
  checks.push(
    'Orchestra workers, dependencies and integration survive GUI restart without duplicate lanes',
  );

  const crashed = await h.invoke('create_session', { workspace });
  await h.invoke('send_message', {
    id: crashed.id,
    text: 'DAEMON_TURN_FIXTURE crash check',
    options,
  });
  await until(
    async () => (await messages(crashed.id)).at(-1)?.text.includes('DAEMON_PARTIAL'),
    'crash partial',
  );
  const owned = await until(async () => {
    try {
      return JSON.parse(await readFile(path.join(workspace, 'daemon-owned-pids.json'), 'utf8'));
    } catch {
      return false;
    }
  }, 'owned process identities');
  unrelated = spawn(
    'powershell.exe',
    ['-NoProfile', '-NonInteractive', '-Command', 'Start-Sleep -Seconds 90'],
    { windowsHide: true, stdio: 'ignore' },
  );
  const pid = (await client.call('__status')).pid;
  execFileSync('taskkill', ['/PID', String(pid), '/F'], { windowsHide: true, stdio: 'ignore' });
  await until(async () => owned.every((p) => !alive(p)), 'kernel job cleanup');
  assert(alive(unrelated.pid));
  await reopen();
  assert.notEqual((await client.call('__status')).pid, pid);
  assert.equal(await status(crashed.id), 'interrupted');
  assert((await messages(crashed.id)).at(-1).text.includes('DAEMON_PARTIAL'));
  await sleep(1000);
  assert.equal(
    (await readFile(path.join(workspace, 'daemon-effect.txt'), 'utf8')).trim().split(/\r?\n/)
      .length,
    2,
  );
  checks.push(
    'Hard service crash kills owned descendants, preserves partial output, leaves unrelated process alive and never replays work',
  );
  await writeFile(
    '.codex/verification/010-native-daemon.json',
    JSON.stringify({ passed: true, checks, data }, null, 2),
  );
  console.log(JSON.stringify({ passed: true, checks }, null, 2));
} finally {
  unrelated?.kill();
  await h.close();
}
