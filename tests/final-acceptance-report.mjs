// Consolidate current release gates separately from historical improvement evidence.
import assert from 'node:assert/strict';
import { readFile, writeFile, readdir } from 'node:fs/promises';
import { createHash } from 'node:crypto';
const root = '.impeccable/final-acceptance';
const json = async (file) => JSON.parse(await readFile(file, 'utf8'));
const hash = (buffer) => createHash('sha256').update(buffer).digest('hex');
const median = (values) => {
  const a = [...values].sort((a, b) => a - b);
  return (a[Math.floor((a.length - 1) / 2)] + a[Math.ceil((a.length - 1) / 2)]) / 2;
};
const blocks = [];
for (const name of ['baseline-a', 'candidate-a', 'baseline-b', 'candidate-b']) {
  const record = await json(`${root}/${name}.json`);
  assert.equal(record.launches.length, 20);
  assert(
    record.launches.every(
      (launch) =>
        launch.sha256 === record.sha256 &&
        launch.windowState.reducedMotion &&
        launch.windowState.focused &&
        launch.windowState.visibility === 'visible',
    ),
  );
  assert.equal(record.samples.length, 20);
  assert(
    record.completedAt &&
      record.samples.every((s) => s.size === 5000 && s.kind === 'mixed' && s.articles === 5000),
  );
  const metrics = Object.fromEntries(
    ['openMs', 'typingTaskMs', 'typingMs', 'streamMs', 'streamTaskMs', 'heapBytes'].map((key) => [
      key,
      median(record.samples.map((s) => s[key])),
    ]),
  );
  const frames = record.samples.flatMap((s) => s.scroll.frames).sort((a, b) => a - b);
  const percentile95 = (values) =>
    [...values].sort((a, b) => a - b)[Math.ceil(values.length * 0.95) - 1];
  blocks.push({
    name,
    sha256: record.sha256,
    samples: record.samples.length,
    medians: metrics,
    openingRangeMs: [
      Math.min(...record.samples.map((s) => s.openMs)),
      Math.max(...record.samples.map((s) => s.openMs)),
    ],
    openingP95Ms: percentile95(record.samples.map((s) => s.openMs)),
    typingTaskP95Ms: percentile95(record.samples.map((s) => s.typingTaskMs)),
    windowState: record.launches[0].windowState,
    scrollFrameP95Ms: frames[Math.ceil(frames.length * 0.95) - 1],
    startupObservationMs: record.startupMs,
  });
}
assert.equal(blocks[0].sha256, blocks[2].sha256);
assert.equal(blocks[1].sha256, blocks[3].sha256);
assert(
  blocks.every(
    (block) => JSON.stringify(block.windowState) === JSON.stringify(blocks[0].windowState),
  ),
);
const comparisons = [0, 2].map((index) => {
  const before = blocks[index],
    after = blocks[index + 1];
  const changes = Object.fromEntries(
    ['openMs', 'typingTaskMs'].map((key) => [
      key,
      Number(((after.medians[key] / before.medians[key] - 1) * 100).toFixed(1)),
    ]),
  );
  return {
    baseline: before.name,
    candidate: after.name,
    changePercent: changes,
    passes: Object.values(changes).every((value) => value <= 10),
  };
});
const acceptance = await json(`${root}/acceptance-results.json`);
assert(acceptance.passed && acceptance.checks.length === 8);
assert.equal(acceptance.sha256, blocks[1].sha256);
assert.equal(hash(await readFile(`${root}/candidate/lumen.exe`)), acceptance.sha256);
for (const name of await readdir('dist/assets')) {
  assert(!name.includes('VirtualWindow'));
  if (name.endsWith('.js'))
    assert(
      !/virtual-window-prototype|transcriptExperiment/.test(
        await readFile(`dist/assets/${name}`, 'utf8'),
      ),
    );
}
const historical = [];
for (const file of [
  'performance',
  'initial-open',
  'turn-latency',
  'metadata-shutdown',
  'context-graph-budget',
]) {
  const source = `docs/data/${file}-0.8-results.json`;
  historical.push({
    source,
    sha256: hash(await readFile(source)),
    scope: 'Historical checkpoint; not a measurement of this final candidate',
  });
}
const onboarding = await json('.impeccable/review/native-onboarding-results.json');
const composer = await json('.impeccable/review/native-composer-results.json');
const execution = await json('.impeccable/review/native-execution-results.json');
const warm = await json(`${root}/excluded-warm-baseline.json`);
assert.equal(onboarding.checks.length, 3);
assert.equal(composer.checks.length, 7);
assert.equal(execution.checks.length, 8);
const report = {
  date: '2026-11-12',
  sourceBase: '01bf144',
  protocol: 'docs/final-acceptance-0.8.md',
  liveCalls: 0,
  historicalLiveSlotsSpent: 13,
  build:
    'Windows x64 optimized release, embedded frontend, isolated benchmark identity/profile; not a distributable package',
  blocks,
  comparisons,
  regressionGate: comparisons.every((c) => c.passes)
    ? 'passed'
    : comparisons.some((c) => c.passes)
      ? 'inconclusive'
      : 'regression',
  nativeRelease: acceptance,
  nativeDebug: {
    onboarding: onboarding.checks,
    composer: composer.checks,
    execution: execution.checks,
  },
  historical,
  validation: { rustPassed: 180, rustOptInIgnored: 14, frontendUnitPassed: 72, browserPassed: 40 },
  excludedWarmSeries: {
    record: 'excluded-warm-baseline.json',
    samples: warm.samples.length,
    sha256: warm.sha256,
    openingMs: warm.samples.map((sample) => sample.openMs),
    reason:
      'Repeated openings in one baseline process were nonstationary before candidate measurement; retained and excluded from the fresh-process regression series',
  },
  candidates: [
    {
      name: 'Repeated openings in one process',
      status: 'unresolved: baseline nonstationarity; not certified by the fresh-process comparison',
    },
    { name: 'Transcript/composer isolation', status: 'integrated', evidence: 'performance-0.8.md' },
    { name: 'Literal paragraph fast path', status: 'integrated', evidence: 'initial-open-0.8.md' },
    {
      name: 'Windows metadata Job Object shutdown',
      status: 'integrated',
      evidence: 'metadata-shutdown-0.8.md',
    },
    {
      name: 'Windowed transcript prototype',
      status: 'excluded: historical find/selection/access gates failed',
      evidence: 'initial-open-0.8.md',
    },
    {
      name: 'Broader MessageView memoization',
      status: 'excluded: secondary scroll results unresolved; narrowed change integrated',
      evidence: 'performance-0.8.md',
    },
    {
      name: 'Rust Markdown/ANSI rendering',
      status: 'deferred: no sufficient post-fix hotspot/prototype benefit evidence',
    },
    {
      name: 'Warm CLI process pool',
      status: 'deferred: no validated benefit and lifecycle/account/workspace isolation prototype',
    },
    {
      name: 'Positional metadata encoding',
      status: 'deferred: synthetic owned-data saving lacks model comprehension/quality evidence',
    },
    {
      name: 'Graph parser replacement',
      status: 'deferred: current scoped corpus passes; full semantics remain unsupported',
    },
    {
      name: 'Global input/output reservation',
      status: 'deferred: no validated allocation across native tool/history/provider paths',
    },
  ],
  limits: [
    'No cold-start, universal absolute timing, provider TTFT or billed-token claim.',
    'Synthetic 5000-message regression gate, not universal performance or memory acceptance.',
    'Private memory/heap and frame timings are secondary, not INP or memory-saving certification.',
    'Native macOS/Linux GUI, new audible screen-reader tutorial review and broader model-quality/live multiaccount tests remain unverified. CI is separate.',
    'Signing and other owner exclusions remain unchanged; integration/version/packages/release are tranche 18.',
  ],
};
await writeFile(
  'docs/data/final-acceptance-0.8-results.json',
  JSON.stringify(report, null, 2) + '\n',
);
console.log(
  JSON.stringify(
    {
      regressionGate: report.regressionGate,
      blocks,
      comparisons,
      nativeReleaseChecks: acceptance.checks.length,
    },
    null,
    2,
  ),
);
