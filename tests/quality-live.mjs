// Explicitly bounded experiment. Re-running never spends a reserved slot again.
import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile, mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { nativeHarness } from './native-harness.mjs';
import { attachLiveAccounts } from './quality-live-accounts.mjs';
import { appendValidation, ownSessionUsage } from './quality-evidence.mjs';
import { prepareProject, checkProject, command, repo } from './quality-projects.mjs';

assert.equal(process.env.LUMEN_LIVE_QUALITY, '1', 'Live inference must be explicitly enabled');
const output = path.join(repo, '.impeccable/experiments');
await mkdir(output, { recursive: true });
const ledgerPath = path.join(output, 'quality-live-budget.json');
let ledger;
try {
  ledger = JSON.parse(await readFile(ledgerPath, 'utf8'));
} catch (error) {
  if (error.code !== 'ENOENT') throw error;
  ledger = { startedAt: new Date().toISOString(), maxPerTaskProvider: 6, slots: [], blocked: {} };
}
const persist = () => writeFile(ledgerPath, JSON.stringify(ledger, null, 2));
const hash = async (file) =>
  createHash('sha256')
    .update(await readFile(file))
    .digest('hex');
const baselineExe = path.join(output, 'quality-baseline/lumen.exe');
const candidateExe = path.join(repo, 'src-tauri/target/debug/lumen.exe');
const baselineHash = await hash(baselineExe);
assert.equal(baselineHash, '4257bd94bea025b001db99c8d6e6778a753f614cb8a31c12ae258a45ae389ccc');
const builds = {
  A: { path: baselineExe, sha256: baselineHash },
  B: { path: candidateExe, sha256: await hash(candidateExe) },
};
ledger.builds ??= builds;
assert.deepEqual(ledger.builds, builds, 'Do not mix candidate revisions in a started experiment');
await persist();
const apps = {};
const homes = {};
const projects = await mkdtemp(path.join(tmpdir(), 'lumen-quality-tasks-'));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let stopping = false;
process.on('SIGINT', () => {
  stopping = true;
});
process.on('SIGTERM', () => {
  stopping = true;
});

// Test the actual external TEMP placement before spending any remaining slot.
for (const kind of ['react', 'godot']) {
  const oracle = path.join(projects, `preflight-${kind}`);
  await prepareProject(kind, oracle, true);
  const checks = await checkProject(kind, oracle);
  assert(
    checks.every((c) => c.passed),
    `External TEMP ${kind} reference checker failed`,
  );
}

async function git(args, directory) {
  const result = await command('git', args, directory);
  assert.equal(result.code, 0, result.stderr);
  return result.stdout.trim();
}
async function turn(h, session, account, text) {
  const started = performance.now();
  const plan = await h.invoke('send_message', {
    id: session.id,
    text,
    options: {
      mode: account.provider,
      accountId: account.account.id,
      model: account.model.id,
      effort: account.effort,
      accessMode: 'autopilot',
      autoReview: false,
      balanceUsage: false,
    },
  });
  for (;;) {
    const state = (await h.invoke('list_sessions', { search: '', offset: 0 })).find(
      (s) => s.id === session.id,
    );
    const messages = await h.invoke('get_messages', { id: session.id });
    const latest = messages.at(-1);
    const pending =
      (await h.invoke('pending_questions')).length > 0 ||
      latest?.activity?.some((a) => ['question', 'approval'].includes(a.kind));
    if (['completed', 'error', 'interrupted'].includes(state?.status))
      return { plan, status: state.status, messages, elapsedMs: performance.now() - started };
    if (stopping || pending || performance.now() - started > 240000) {
      await h.invoke('cancel_run', { id: session.id });
      await sleep(1000);
      return {
        plan,
        status: stopping ? 'interrupted' : pending ? 'pending-input' : 'timeout',
        messages,
        elapsedMs: performance.now() - started,
      };
    }
    await sleep(500);
  }
}

try {
  for (const arm of ['A', 'B']) {
    apps[arm] = await nativeHarness(`quality-live-${arm}`, {
      executable: builds[arm].path,
      offline: false,
    });
    homes[arm] = await attachLiveAccounts(apps[arm]);
    const settings = await apps[arm].invoke('get_settings');
    await apps[arm].invoke('save_settings', {
      value: {
        ...settings,
        verifyChanges: false,
        selfHeal: false,
        autoFallback: false,
        orchestra: false,
        closeToTray: false,
        repoContext: false,
        localAssistant: { ...settings.localAssistant, enabled: false },
      },
    });
  }
  for (const kind of ['react', 'godot']) {
    for (const provider of ['claude', 'gpt']) {
      const accounts = Object.fromEntries(
        ['A', 'B'].map((arm) => [arm, homes[arm].accounts.find((a) => a.provider === provider)]),
      );
      if (!accounts.A?.ready || !accounts.B?.ready) {
        ledger.blocked[provider] =
          'Authentication/catalog prerequisite unavailable; no inference retry';
        await persist();
        continue;
      }
      assert.equal(accounts.A.model.id, accounts.B.model.id);
      assert.equal(accounts.A.effort, accounts.B.effort);
      for (let pair = 1; pair <= 3; pair++) {
        for (const arm of pair % 2 ? ['A', 'B'] : ['B', 'A']) {
          const key = `${kind}/${provider}/${pair}/${arm}`;
          if (stopping || ledger.blocked[provider] || ledger.slots.some((s) => s.key === key))
            continue;
          assert(ledger.slots.filter((s) => s.kind === kind && s.provider === provider).length < 6);
          const h = apps[arm],
            account = accounts[arm];
          const slot = {
            key,
            kind,
            provider,
            pair,
            arm,
            state: 'reserved',
            at: new Date().toISOString(),
            model: account.model.id,
            effort: account.effort,
            turns: [],
          };
          ledger.slots.push(slot);
          await persist(); // Failed/abandoned attempts consume a slot, too.
          process.stdout.write(`START ${key}\n`);
          const directory = path.join(projects, key.replaceAll('/', '-'));
          try {
            await prepareProject(kind, directory);
            await git(['init', '--quiet'], directory);
            await git(['add', '.'], directory);
            await git(
              [
                '-c',
                'user.name=Lumen fixture',
                '-c',
                'user.email=fixture@localhost',
                'commit',
                '--quiet',
                '-m',
                'Independent fixture baseline',
              ],
              directory,
            );
            const allowed =
              kind === 'react'
                ? ['counter.ts', 'CounterView.tsx']
                : ['CounterState.cs', 'CounterPanel.gd'];
            const ui = allowed[1];
            const originalUi = await hash(path.join(directory, ui));
            const session = await h.invoke('create_session', { workspace: directory });
            h.validationWorkspace = session.workspace;
            slot.project = directory;
            slot.session = session.id;
            const start = performance.now();
            const first = `This is an isolated ${kind === 'react' ? 'React/TypeScript' : 'Godot .NET 2D C# + GDScript'} coding fixture. Implement only the core arithmetic fix in ${allowed[0]}: clamp the resulting value to the inclusive range [0, limit], including large positive/negative steps and zero limit. Preserve public signatures. In this first step leave ${ui} unchanged. Work autonomously with your native file tools. No subagents, internet, dependency installation, external projects, config or test changes; do not create documentation or commit. Finish concisely with actual checks. Existing dependencies/SDK are installed. All paths belong to this temporary workspace.`;
            const result = await turn(h, session, account, first);
            slot.turns.push({ status: result.status, elapsedMs: result.elapsedMs });
            await persist();
            slot.firstStepScopePreserved = originalUi === (await hash(path.join(directory, ui)));
            let messages = result.messages;
            if (result.status === 'completed') {
              slot.intermediateChecks = await checkProject(kind, directory);
              const failureDetail =
                'Remaining requirement: Hit must be disabled at the limit 3 and enabled again after Reset; retain the exact Value label/output (0 / 3 then 3 / 3), node names and existing event hookups. ' +
                slot.intermediateChecks
                  .map((c) => `${c.name}: ${c.passed ? 'passed' : 'failed'}`)
                  .join('; ');
              appendValidation(
                h,
                session.id,
                'Independent counter check',
                slot.intermediateChecks.every((c) => c.passed) ? 'passed' : 'failed',
                failureDetail,
              );
              const second = await turn(
                h,
                session,
                account,
                'Finish the remaining counter work using the current Lumen-recorded independent check evidence. Preserve the earlier arithmetic requirements, signatures and labels. You may now edit the UI file as well as the arithmetic file. Implement and verify autonomously; use existing tools/dependencies only, no subagents, internet, external projects, config/test changes or commits. Do not ask for confirmation. Return the actual result and any unavailable checks.',
              );
              slot.turns.push({ status: second.status, elapsedMs: second.elapsedMs });
              await persist();
              messages = second.messages;
            }
            slot.finalChecks = await checkProject(kind, directory);
            slot.elapsedMs = performance.now() - start;
            const changed = (await git(['diff', '--name-only', 'HEAD'], directory))
              .split('\n')
              .filter(Boolean);
            const untracked = (await git(['ls-files', '--others', '--exclude-standard'], directory))
              .split('\n')
              .filter(Boolean);
            slot.changed = changed;
            slot.unrelated = [...changed, ...untracked].filter((file) => !allowed.includes(file));
            const nativeIds = messages
              .flatMap((m) =>
                m.activity
                  .filter((a) => a.kind === 'native')
                  .map((a) => {
                    try {
                      return JSON.parse(a.text).session;
                    } catch {
                      return null;
                    }
                  }),
              )
              .filter(Boolean);
            slot.usage = await ownSessionUsage(account, nativeIds);
            slot.passed =
              slot.turns.length === 2 &&
              slot.turns.every((t) => t.status === 'completed') &&
              slot.finalChecks.every((c) => c.passed) &&
              slot.unrelated.length === 0 &&
              slot.firstStepScopePreserved;
            slot.state = 'finished';
            const terminal = messages.at(-1);
            if (slot.turns.some((t) => t.status === 'error')) {
              const nativeErrors = (terminal?.activity || [])
                .filter((a) => a.kind === 'error')
                .map((a) => a.text)
                .join(' ');
              const errorText = `${nativeErrors} ${terminal?.text || ''}`;
              slot.failureClass = /quota|rate.limit|usage.limit|capacity|exhausted|429/i.test(
                errorText,
              )
                ? 'quota'
                : /login|authenticat|sign.in|credential/i.test(errorText)
                  ? 'authentication'
                  : 'native-error';
              // Preserve only a class, never raw provider output or private account data.
              if (['quota', 'authentication'].includes(slot.failureClass))
                ledger.blocked[provider] = slot.failureClass;
            }
          } catch (error) {
            slot.state = 'failed';
            slot.passed = false;
            slot.failureClass = 'harness-or-prerequisite';
            // Paths are our generated projects; avoid arbitrary native stderr in public artifacts.
            slot.error = error?.code || error?.name || 'Error';
            ledger.blocked[provider] =
              'Harness/prerequisite failure; inspect before any further spending';
          }
          await persist();
          process.stdout.write(
            JSON.stringify({
              key,
              state: slot.state,
              passed: slot.passed,
              turns: slot.turns,
              failureClass: slot.failureClass,
              usage: slot.usage,
            }) + '\n',
          );
        }
      }
    }
  }
  ledger.finishedAt = new Date().toISOString();
  await persist();
} finally {
  for (const arm of ['B', 'A']) {
    await apps[arm]?.close();
    await homes[arm]?.cleanup();
  }
}
