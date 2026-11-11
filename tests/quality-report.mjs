// Publish aggregate experiment evidence only; no account IDs, native logs or task paths.
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { repo } from './quality-projects.mjs';

const ledger = JSON.parse(
  await readFile(path.join(repo, '.impeccable/experiments/quality-live-budget.json'), 'utf8'),
);
assert(
  ledger.finishedAt && !ledger.slots.some((s) => s.state === 'reserved'),
  'Live experiment has unfinished slots',
);
const median = (values) => {
  if (!values.length) return null;
  const sorted = [...values].sort((a, b) => a - b);
  return (
    (sorted[Math.floor((sorted.length - 1) / 2)] + sorted[Math.ceil((sorted.length - 1) / 2)]) / 2
  );
};
const groups = [];
for (const kind of ['react', 'godot'])
  for (const provider of ['claude', 'gpt']) {
    const slots = ledger.slots.filter((s) => s.kind === kind && s.provider === provider);
    groups.push({
      task: kind,
      provider,
      spentSlots: slots.length,
      maximumSlots: 6,
      blocked: ledger.blocked[provider] || null,
      arms: ['A', 'B'].map((arm) => {
        const attempts = slots.filter((s) => s.arm === arm);
        const finished = attempts.filter((s) => s.state === 'finished' && !s.failureClass);
        return {
          arm,
          attempts: attempts.length,
          evaluable: finished.length,
          passed: finished.filter((s) => s.passed).length,
          medianTaskMs: median(finished.map((s) => s.elapsedMs)),
          medianInputTokens: median(
            finished.map((s) => s.usage?.inputTokens).filter(Number.isFinite),
          ),
          medianOutputTokens: median(
            finished.map((s) => s.usage?.outputTokens).filter(Number.isFinite),
          ),
          medianCacheReadTokens: median(
            finished.map((s) => s.usage?.cacheReadTokens).filter(Number.isFinite),
          ),
          unmeasuredUsageAttempts: attempts.filter((s) => !Number.isFinite(s.usage?.inputTokens))
            .length,
        };
      }),
    });
  }
const result = {
  at: new Date().toISOString(),
  finishedAt: ledger.finishedAt,
  version: '0.8 working tree, not a release',
  builds: Object.fromEntries(
    Object.entries(ledger.builds).map(([arm, build]) => [arm, build.sha256]),
  ),
  local: {
    corpusBaseline: '2/14',
    corpusCandidate: '14/14',
    interpretation:
      'Fixed deterministic routing corpus; not model intelligence or live Auto performance',
  },
  protocol: {
    arms: {
      A: 'Preserved pre-quality baseline',
      B: 'Changed Lumen-owned context on direct native resume',
    },
    models:
      'Matched per provider, low effort; direct native mode, no classifier/reviewer/repair calls',
    turns: 'Two user turns per full task, no replacement of failed/invalidated slots',
    checks: 'Independent compile and React UI / Godot C#-GDScript headless runtime',
    time: 'Full observed task including independent checks; excludes app/project setup and preflight',
    tokens:
      'Cumulative exact owned native-session logs, includes native tool turns; Codex input includes cached tokens. Monetary charges unavailable.',
    limitations: [
      'Small synthetic tasks, low effort and provider cache variability; no universal superiority claim',
      'Not a comparison with official GUI apps; not live Auto, cross-audit or two-Claude acceptance',
      'Windows debug; no release latency/memory baseline or non-Windows GUI acceptance',
    ],
  },
  budget: {
    spentSlots: ledger.slots.length,
    maximumPerTaskProvider: 6,
    extraSlots: 0,
    invalidated: ledger.slots
      .filter((s) => s.state === 'invalidated')
      .map((s) => ({ key: s.key, reason: s.failureClass })),
    blockedProviders: ledger.blocked,
    checkerRepair: ledger.oracleRepair || null,
  },
  groups,
  runs: ledger.slots.map((s) => ({
    key: s.key,
    state: s.state,
    passed: !!s.passed,
    failureClass: s.failureClass || null,
    model: s.model,
    effort: s.effort,
    turnStatuses: s.turns.map((t) => t.status),
    observedTurnsAfterInterruptedHarness: s.inferenceTurnsObserved ?? null,
    firstStepScopePreserved: s.firstStepScopePreserved ?? null,
    unrelatedChanges: s.unrelated?.length ?? null,
    finalChecks: s.finalChecks?.map(({ name, passed }) => ({ name, passed })) ?? null,
    elapsedMs: s.elapsedMs ?? null,
    usage: s.usage || { source: 'unavailable' },
  })),
};
await mkdir(path.join(repo, 'docs/data'), { recursive: true });
await writeFile(
  path.join(repo, 'docs/data/quality-0.8-results.json'),
  JSON.stringify(result, null, 2) + '\n',
);
process.stdout.write(JSON.stringify({ budget: result.budget, groups }, null, 2) + '\n');
