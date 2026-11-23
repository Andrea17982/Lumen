// Real Windows IPC/worktrees with offline CLI fixtures; never calls a model.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import path from 'node:path';
import { tmpdir } from 'node:os';
import { nativeHarness } from './native-harness.mjs';

const h = await nativeHarness('orchestra-continuity');
const checks = [];
try {
  const relative = path.relative(tmpdir(), h.data);
  assert(!relative.startsWith('..') && !path.isAbsolute(relative));
  assert(relative.startsWith('lumen-orchestra-continuity-'));
  const git = (...args) =>
    execFileSync('git', ['-C', h.workspace, ...args], {
      windowsHide: true,
      encoding: 'utf8',
    }).trim();
  git('init');
  git(
    '-c',
    'user.name=Fixture',
    '-c',
    'user.email=fixture@example.invalid',
    'commit',
    '--allow-empty',
    '-m',
    'Owned fixture',
  );
  const base = git('write-tree');
  const task = (id, account, extra = {}) => ({
    id,
    title: id,
    detail: 'ORCHESTRA_CONTINUITY_FIXTURE',
    accountId: account.id,
    agent: 'Claude',
    model: 'sonnet',
    effort: 'low',
    difficulty: 'normal',
    writes: true,
    pinned: true,
    status: 'interrupted',
    attempts: 0,
    files: ['owned-worker.txt'],
    dependsOn: [],
    ...extra,
  });
  async function run(name, tasks, started) {
    const account = await h.invoke('add_account', { name, provider: 'claude' });
    const session = await h.invoke('create_session', { workspace: h.workspace });
    const message = randomUUID();
    const state = {
      stage: 'interrupted',
      resumable: true,
      workspace: h.workspace,
      task: 'Continue the approved isolated task.',
      context: 'APPROVED_SCOPE_MARKER',
      executionAuthorized: true,
      orchestrator: account.id,
      orchestratorAgent: 'Claude',
      orchestratorModel: 'sonnet',
      effort: 'low',
      base,
      options: {
        mode: 'claude',
        accountId: account.id,
        model: 'sonnet',
        effort: 'low',
        accessMode: 'autopilot',
        autoReview: false,
      },
      subtasks: tasks(account),
    };
    // Seed a persisted interrupted card only inside this harness-owned database.
    const script =
      'import sqlite3,sys,json; c=sqlite3.connect(sys.argv[1]); c.execute("INSERT INTO messages VALUES(?,?,?,?,?,?)",(sys.argv[2],sys.argv[3],"assistant","",sys.argv[4],int(sys.argv[5]))); c.commit()';
    execFileSync(
      'python',
      [
        '-c',
        script,
        path.join(h.data, 'lumen.db'),
        message,
        session.id,
        JSON.stringify([{ kind: 'orchestra', text: JSON.stringify(state) }]),
        String(Date.now()),
      ],
      { windowsHide: true },
    );
    await h.invoke('orchestra_resume', { id: session.id, message, force: false });
    if (started) await started({ account, session });
    let terminal;
    for (let i = 0; i < 450; i++) {
      terminal = (await h.invoke('list_sessions', { search: '', offset: 0 })).find(
        (row) => row.id === session.id,
      )?.status;
      if (['completed', 'error', 'interrupted'].includes(terminal)) break;
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
    assert(
      ['completed', 'error', 'interrupted'].includes(terminal),
      `Unfinished ${name}: ${terminal}`,
    );
    const messages = await h.invoke('get_messages', { id: session.id });
    const out = messages.at(-1);
    const card = out.activity.findLast((row) => row.kind === 'orchestra');
    assert(card, JSON.stringify(out));
    const result = JSON.parse(card.text);
    const counter = await readFile(
      path.join(h.data, 'accounts', account.id, 'worker-continuity-attempts.txt'),
      'utf8',
    ).catch(() => '');
    return {
      result,
      attempts: counter.trim() ? counter.trim().split(/\r?\n/).length : 0,
      session,
      message,
      terminal,
    };
  }
  const failed = await run('Worker effect then quota', (account) => [
    task('s1', account, { detail: 'ORCHESTRA_CONTINUITY_FIXTURE WORKER_QUOTA_AFTER_EFFECT' }),
    task('s2', account, { dependsOn: ['s1'] }),
  ]);
  assert.equal(failed.attempts, 1);
  assert.equal(failed.result.stage, 'failed');
  assert.equal(failed.result.subtasks[0].attempts, 1);
  assert.match(failed.result.subtasks[0].output, /Preserve worker partial output/);
  assert.match(failed.result.subtasks[0].output, /not retried after observed work/);
  assert.equal(failed.result.subtasks[1].attempts, 0);
  assert.equal(failed.result.subtasks[1].status, 'failed');
  assert.match(failed.result.subtasks[1].output, /Dependency s1/);
  await assert.rejects(
    h.invoke('orchestra_resume', { id: failed.session.id, message: failed.message, force: false }),
  );
  checks.push(
    'Observed worker write and partial output followed by quota: one attempt, blocked dependency, consumed Resume rejected',
  );

  const exhausted = await run('Saved attempts exhausted', (account) => [
    task('s1', account, { attempts: 3 }),
  ]);
  assert.equal(exhausted.attempts, 0);
  assert.equal(exhausted.result.subtasks[0].attempts, 3);
  assert.match(exhausted.result.subtasks[0].output, /Attempt limit/);
  checks.push('Persisted three-attempt limit prevents launching a fourth CLI on Resume');

  const continued = await run('Completed work retained', (account) => [
    task('s1', account, { status: 'done', attempts: 1, output: 'DONE_SEED_MARKER' }),
    task('s2', account, { attempts: 1, dependsOn: ['s1'] }),
  ]);
  assert.equal(continued.attempts, 1);
  assert.equal(continued.result.stage, 'done', JSON.stringify(continued.result));
  assert.equal(continued.result.subtasks[0].output, 'DONE_SEED_MARKER');
  assert.equal(continued.result.subtasks[0].attempts, 1);
  assert.equal(continued.result.subtasks[1].attempts, 2);
  assert.match(continued.result.subtasks[1].output, /WORKER_SCOPE_PRESENT/);
  checks.push(
    'Completed task retained; only interrupted worker resumes with saved scope/account/model/workspace and incremented attempts',
  );
  let ownedPids;
  try {
    const cancelled = await run(
      'Cancelled worker',
      (account) => [
        task('s1', account, { detail: 'ORCHESTRA_CONTINUITY_FIXTURE WORKER_CANCEL' }),
        task('s2', account, { dependsOn: ['s1'] }),
      ],
      async ({ account, session }) => {
        const file = path.join(h.data, 'accounts', account.id, 'worker-cancel-pids.json');
        for (let i = 0; i < 200; i++) {
          try {
            ownedPids = JSON.parse(await readFile(file, 'utf8'));
            break;
          } catch {}
          await new Promise((resolve) => setTimeout(resolve, 100));
        }
        assert(ownedPids?.length === 2, 'Owned child never started');
        await h.invoke('cancel_run', { id: session.id });
      },
    );
    assert.equal(cancelled.terminal, 'interrupted');
    assert.equal(cancelled.attempts, 1);
    assert.equal(cancelled.result.subtasks[1].attempts, 0);
    const alive = execFileSync(
      'powershell.exe',
      [
        '-NoProfile',
        '-NonInteractive',
        '-Command',
        `@(Get-Process -Id ${ownedPids.join(',')} -ErrorAction SilentlyContinue).Count`,
      ],
      { windowsHide: true, encoding: 'utf8' },
    ).trim();
    assert.equal(alive, '0', 'Stop left an owned worker/descendant running');
    assert.equal(git('worktree', 'list', '--porcelain').match(/^worktree /gm).length, 1);
    checks.push(
      'Stop waits for owned worker/child cleanup before removing lanes; no dependent or retry starts',
    );
  } finally {
    // Clean only the two processes created by this fixture, including after a failed baseline.
    for (const pid of ownedPids || []) {
      try {
        execFileSync('taskkill.exe', ['/PID', String(pid), '/T', '/F'], {
          windowsHide: true,
          stdio: 'ignore',
        });
      } catch {}
    }
  }
  await mkdir('.impeccable/review', { recursive: true });
  await writeFile(
    '.impeccable/review/native-orchestra-continuity-results.json',
    JSON.stringify({ offline: true, liveCalls: 0, checks }, null, 2),
  );
  console.log(JSON.stringify({ passed: checks.length, checks }, null, 2));
} finally {
  await h.close();
}
