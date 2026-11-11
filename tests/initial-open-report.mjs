// Publish aggregate evidence only, without account identifiers or local profile paths.
import { readFile, writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
import path from 'node:path';

const root = path.resolve(process.env.LUMEN_PERF_ROOT || '.impeccable/initial-open');
const result = JSON.parse(await readFile(path.join(root, 'experiment-results.json'), 'utf8'));
assert(result.completedAt, 'Incomplete comparison');
assert.equal(result.repetitions, 20);
const median = (values) => {
  const s = [...values].sort((a, b) => a - b);
  return (s[Math.floor((s.length - 1) / 2)] + s[Math.ceil((s.length - 1) / 2)]) / 2;
};
const stats = (values) => {
  assert(values.length && values.every(Number.isFinite));
  const s = [...values].sort((a, b) => a - b);
  return { median: median(s), p95: s[Math.ceil(s.length * 0.95) - 1], min: s[0], max: s.at(-1) };
};
const workloads = [];
for (const size of [100, 1000, 5000])
  for (const kind of ['short', 'mixed']) {
    const arms = {};
    const rows = (arm) =>
      result.samples.filter((s) => s.arm === arm && s.size === size && s.kind === kind);
    for (const arm of ['baseline', 'plain', 'virtual']) {
      const samples = rows(arm);
      assert.equal(samples.length, 20);
      assert.equal(new Set(samples.map((s) => s.repetition)).size, 20);
      arms[arm] = {
        samples: samples.length,
        ...Object.fromEntries(
          [
            'openMs',
            'taskMs',
            'scriptMs',
            'layoutMs',
            'typingTaskMs',
            'heapBytes',
            'nodes',
            'articles',
          ].map((field) => [field, stats(samples.map((s) => s[field]))]),
        ),
        allMessagesMounted: samples.every((s) => s.articles === size),
        longTaskCount: samples.reduce((n, s) => n + s.longTasks.length, 0),
        longestTaskMs: Math.max(0, ...samples.flatMap((s) => s.longTasks)),
        visibleFocusedSamples: samples.filter((s) => s.focused && s.visibility === 'visible')
          .length,
        privateProcessProbe: samples[0].processes,
        ipcReadProbe: samples[0].ipcProbe,
      };
    }
    workloads.push({
      size,
      kind,
      ...arms,
      openingMedianReduction: 1 - arms.plain.openMs.median / arms.baseline.openMs.median,
      pairedOpeningReduction: stats(
        rows('plain').map(
          (s) => 1 - s.openMs / rows('baseline').find((b) => b.repetition === s.repetition).openMs,
        ),
      ),
      typingMedianChange: arms.plain.typingTaskMs.median / arms.baseline.typingTaskMs.median - 1,
    });
  }
assert.equal(result.samples.length, 360);
const [baseline, plain, virtual] = ['baseline', 'plain', 'virtual'].map((arm) =>
  result.gates.find((g) => g.arm === arm),
);
const preserved =
  ['htmlHash', 'axHash', 'selectionHash'].every((key) => baseline[key] === plain[key]) &&
  ['axFirst', 'axLast', 'findFirst', 'selectionFirst', 'selectionLast'].every(
    (key) => baseline[key] && plain[key],
  );
const primary = workloads.find((w) => w.size === 5000 && w.kind === 'mixed');
const secondary = JSON.parse(await readFile(path.join(root, 'secondary-results.json'), 'utf8'));
assert(secondary.completedAt);
assert.equal(secondary.samples.length, 80);
const secondaryWorkloads = [100, 1000].map((size) => {
  const byArm = {};
  for (const arm of ['baseline', 'plain']) {
    const rows = secondary.samples.filter((s) => s.size === size && s.arm === arm);
    assert.equal(rows.length, 20);
    byArm[arm] = {
      openMs: stats(rows.map((s) => s.openMs)),
      typingTaskMs: stats(rows.map((s) => s.typingTaskMs)),
    };
  }
  return { size, kind: 'mixed', ...byArm };
});
const confirmation = JSON.parse(
  await readFile(path.join(root, 'confirmation-results.json'), 'utf8'),
);
const stock = JSON.parse(await readFile(path.join(root, 'stock-results.json'), 'utf8'));
const acceptance = JSON.parse(await readFile(path.join(root, 'acceptance-results.json'), 'utf8'));
const asymmetric = JSON.parse(
  await readFile(path.join(root, 'confirmation-asymmetric-results.json'), 'utf8'),
);
const motionDiagnostic = JSON.parse(
  await readFile(path.join(root, 'motion-diagnostic-results.json'), 'utf8'),
);
assert.equal(asymmetric.samples.length, 40);
assert(motionDiagnostic.observations[0].behaviors.includes('smooth'));
assert(motionDiagnostic.observations[1].behaviors.includes('instant'));
assert(confirmation.completedAt);
assert.equal(confirmation.samples.length, 40);
assert.equal(confirmation.hashes.baseline, result.sha256);
assert.equal(confirmation.hashes.candidate, stock.sha256);
assert.equal(acceptance.sha256, stock.sha256);
assert(acceptance.passed && acceptance.checks.length === 8);
for (const key of [
  'noPrototypeAssets',
  'queryCannotEnablePrototype',
  'full5000HtmlMatches',
  'selection1000Matches',
  'nativeFind1000',
  'targetedAx1000Matches',
])
  assert.equal(stock[key], true);
const finalArms = {};
for (const arm of ['baseline', 'candidate']) {
  const rows = confirmation.samples.filter((s) => s.arm === arm);
  assert.equal(rows.length, 20);
  assert(rows.every((s) => s.focused && s.visibility === 'visible'));
  finalArms[arm] = {
    openMs: stats(rows.map((s) => s.openMs)),
    taskMs: stats(rows.map((s) => s.taskMs)),
    typingTaskMs: stats(rows.map((s) => s.typingTaskMs)),
  };
}
const finalReduction = 1 - finalArms.candidate.openMs.median / finalArms.baseline.openMs.median;
const finalTypingChange =
  finalArms.candidate.typingTaskMs.median / finalArms.baseline.typingTaskMs.median - 1;
assert(finalTypingChange <= 0.05, 'Investigate a final typing regression above 5%');
const finalPairedReduction = stats(
  confirmation.samples
    .filter((s) => s.arm === 'candidate')
    .map(
      (s) =>
        1 -
        s.openMs /
          confirmation.samples.find((b) => b.arm === 'baseline' && b.repetition === s.repetition)
            .openMs,
    ),
);
assert(
  finalReduction >= 0.2 && finalPairedReduction.median >= 0.2,
  'Final ordinary build must also pass',
);
assert(preserved);
assert(primary.openingMedianReduction >= 0.2 && primary.pairedOpeningReduction.median >= 0.2);
assert(!virtual.findFirst && !virtual.selectionFirst && !virtual.axFirst);
const report = {
  at: result.at,
  completedAt: result.completedAt,
  protocol: 'docs/initial-open-0.8.md',
  experimentSha256: result.sha256,
  environment: {
    webview: result.environment.webview.product,
    userAgent: result.environment.webview.userAgent,
    window: result.environment.window,
    platform: 'Windows x64 release / embedded assets / isolated synthetic fixture',
  },
  primary: {
    size: 5000,
    kind: 'mixed',
    threshold: 0.2,
    baselineMedianMs: primary.baseline.openMs.median,
    candidateMedianMs: primary.plain.openMs.median,
    reduction: primary.openingMedianReduction,
    pairedReduction: primary.pairedOpeningReduction,
    passed: true,
  },
  workloads,
  secondaryInvestigation: {
    samples: 80,
    sha256: secondary.sha256,
    workloads: secondaryWorkloads,
    conclusion:
      'Original small-chat opening p95 increase and 1000-message typing increase do not repeat. A separate roughly 1 ms typing increase on 100 messages is not reproduced in the main series; no general typing improvement is claimed.',
  },
  gates: result.gates,
  finalOrdinaryBuild: {
    at: confirmation.at,
    completedAt: confirmation.completedAt,
    hashes: confirmation.hashes,
    samples: 40,
    ...finalArms,
    reduction: finalReduction,
    typingMedianChange: finalTypingChange,
    pairedReduction: finalPairedReduction,
    stock,
    acceptance,
  },
  excludedConfirmation: {
    samples: asymmetric.samples.length,
    hashes: asymmetric.hashes,
    reason:
      'Unequal navigation after reduced-motion emulation mixed smooth and instant scrolling. Original observations retained locally, replaced by 20 equally prepared launch pairs.',
    timing: Object.fromEntries(
      ['baseline', 'candidate'].map((arm) => {
        const rows = asymmetric.samples.filter((s) => s.arm === arm);
        return [
          arm,
          {
            openMs: stats(rows.map((s) => s.openMs)),
            typingTaskMs: stats(rows.map((s) => s.typingTaskMs)),
          },
        ];
      }),
    ),
    diagnostic: motionDiagnostic,
  },
  decision: {
    plainParagraph:
      'Accepted: opening gate and identical HTML, native selection and targeted AX evidence',
    virtualization:
      'Excluded: only the tail is mounted; historical find, selection and AX content fail. Its timing is not full-transcript readiness.',
  },
  limits: [
    '20 paired warm openings per workload/arm in one experimental binary; no cold OS/process startup or inference latency claim.',
    'HTML equivalence covers all 5000 mixed messages. Native selection/find and targeted first/last paragraph AX evidence cover 1000 mixed messages, not full screen-reader certification.',
    'Two exploratory 18-sample pilots were stopped during unbounded 5000-message full-AX/selection diagnostics; excluded from the fixed 360-sample timing series, retained locally.',
    'Memory/process and IPC probes are single first-repetition observations per workload/arm; do not establish peak memory or IPC speedup.',
    'No measured scroll improvement is claimed; complete DOM and layout remain unchanged for the accepted path.',
  ],
  liveInferenceCalls: 0,
  previousLiveSlotsSpent: 13,
};
await writeFile('docs/data/initial-open-0.8-results.json', JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify(report.primary));
