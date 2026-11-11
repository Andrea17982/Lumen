import { readFile, writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';

const root = '.impeccable/performance/';
const load = async (name) => JSON.parse(await readFile(`${root}${name}-results.json`, 'utf8'));
const baseline = await load('baseline'),
  candidate = await load('candidate'),
  confirmation = await load('confirmation');
for (const r of [baseline, candidate, confirmation])
  assert(r.completedAt, 'Incomplete measurement series');
const percentile = (values, p) => {
  const s = values.filter(Number.isFinite).sort((a, b) => a - b);
  return s.length ? s[Math.max(0, Math.ceil(s.length * p) - 1)] : null;
};
const median = (values) => {
  const s = values.filter(Number.isFinite).sort((a, b) => a - b);
  return s.length
    ? (s[Math.floor((s.length - 1) / 2)] + s[Math.ceil((s.length - 1) / 2)]) / 2
    : null;
};
const stats = (values) => ({
  median: median(values),
  p95: percentile(values, 0.95),
  min: percentile(values, 0),
  max: percentile(values, 1),
});
const summarize = (rows) => ({
  samples: rows.length,
  openMs: stats(rows.map((s) => s.openMs)),
  typingTaskMs: stats(rows.map((s) => s.typingTaskMs)),
  typingMs: stats(rows.map((s) => s.typingMs)),
  typingScriptMs: stats(rows.map((s) => s.typingScriptMs)),
  typingLayoutMs: stats(rows.map((s) => s.typingLayoutMs)),
  streamTaskMs: stats(rows.map((s) => s.streamTaskMs)),
  streamMs: stats(rows.map((s) => s.streamMs)),
  keyAfterFrameProxyMs: stats(rows.flatMap((s) => s.keys)),
  frameIntervalsMs: stats(rows.flatMap((s) => s.frames)),
  scrollFrameIntervalsMs: stats(rows.flatMap((s) => s.scroll.frames)),
  longTasks: rows.reduce((n, s) => n + s.longTasks.length, 0),
  longTaskMaxMs: percentile(
    rows.flatMap((s) => s.longTasks),
    1,
  ),
  domNodes: stats(rows.map((s) => s.domNodes)),
  heapBytes: stats(rows.map((s) => s.heapBytes)),
  privateProcessBytes: stats(rows.map((s) => s.processes?.privateBytes)),
  privateProcessSamples: rows.filter((s) => s.processes).length,
  processCounts: [...new Set(rows.filter((s) => s.processes).map((s) => s.processes.count))],
  allMessagesMounted: rows.every((s) => s.articles === s.size),
  bottomAfterJumps: rows.filter((s) => s.scroll.atBottom).length,
});
const workloads = [];
for (const size of [100, 1000, 5000])
  for (const kind of ['short', 'mixed']) {
    const a = summarize(baseline.samples.filter((s) => s.size === size && s.kind === kind));
    const b = summarize(candidate.samples.filter((s) => s.size === size && s.kind === kind));
    const c = summarize(confirmation.samples.filter((s) => s.size === size && s.kind === kind));
    assert.equal(a.samples, 20);
    assert.equal(b.samples, 20);
    assert.equal(c.samples, 20);
    workloads.push({
      size,
      kind,
      baseline: a,
      candidate: b,
      baselineControl: c,
      typingTaskReduction: 1 - b.typingTaskMs.median / a.typingTaskMs.median,
      openMedianChange: b.openMs.median / a.openMs.median - 1,
      openMedianChangeAgainstControl: b.openMs.median / c.openMs.median - 1,
    });
  }
const primary = workloads.find((w) => w.size === 5000 && w.kind === 'mixed');
const control = summarize(
  confirmation.samples.filter((s) => s.size === 5000 && s.kind === 'mixed'),
);
assert.equal(control.samples, 20);
const primaryGate =
  primary.typingTaskReduction >= 0.2 &&
  1 - primary.candidate.typingTaskMs.median / control.typingTaskMs.median >= 0.2;
let acceptance;
try {
  acceptance = JSON.parse(await readFile(root + 'acceptance-results.json', 'utf8'));
} catch {
  acceptance = { passed: false, scope: 'Not yet verified' };
}
assert.equal(
  acceptance.sha256,
  candidate.sha256,
  'Native acceptance belongs to another candidate build',
);
const scrollInvestigation = JSON.parse(await readFile(root + 'scroll-investigation.json', 'utf8'));
const scrollPairs = [0, 2].map((i) => {
  const a = scrollInvestigation.results[i],
    b = scrollInvestigation.results[i + 1];
  return {
    baselineP95Ms: a.p95Ms,
    candidateP95Ms: b.p95Ms,
    change: b.p95Ms / a.p95Ms - 1,
    domIdentical: a.domHash === b.domHash,
    focused: a.windowState?.focused && b.windowState?.focused,
  };
});
async function optional(name) {
  try {
    return JSON.parse(await readFile(root + name, 'utf8'));
  } catch (error) {
    if (error.code === 'ENOENT') return null;
    throw error;
  }
}
const previousPrototype = await optional('row-memo-results.json');
const previousScroll = await optional('row-memo-scroll.json');
const result = {
  at: new Date().toISOString(),
  platform: baseline.environment,
  buildMode:
    'Native Windows x64 release, embedded production assets, isolated app identifier/profile, no original accounts',
  hashes: {
    baseline: baseline.sha256,
    candidate: candidate.sha256,
    confirmation: confirmation.sha256,
  },
  ordering:
    'Baseline 20 repetitions/workload, candidate 20/workload, then baseline control 20/workload to investigate observed nonstationary timing. Same fixture, alternating workload order per repetition. Not randomized per-arm.',
  primary: {
    metric: 'UI-thread TaskDuration for fixed 12-key typing batch at 5000 mixed messages',
    thresholdReduction: 0.2,
    baselineMs: primary.baseline.typingTaskMs.median,
    candidateMs: primary.candidate.typingTaskMs.median,
    baselineControlMs: control.typingTaskMs.median,
    reduction: primary.typingTaskReduction,
    gatePassed: primaryGate,
  },
  startupSingleLaunchMs: {
    baseline: baseline.startupMs,
    candidate: candidate.startupMs,
    control: confirmation.startupMs,
  },
  acceptance,
  trialCounts: {
    fullWorkloads:
      baseline.samples.length +
      candidate.samples.length +
      confirmation.samples.length +
      (previousPrototype?.samples.length || 0),
    focusedScroll:
      scrollInvestigation.results.reduce((n, r) => n + r.samples, 0) +
      (previousScroll?.results.reduce((n, r) => n + r.samples, 0) || 0),
    paidInferenceCalls: 0,
  },
  scrollInvestigation,
  scrollPairs,
  excludedPrototype: previousPrototype
    ? {
        sha256: previousPrototype.sha256,
        scope: 'Per-message memoization in addition to transcript isolation',
        primaryTypingTaskMs: summarize(
          previousPrototype.samples.filter((s) => s.size === 5000 && s.kind === 'mixed'),
        ).typingTaskMs,
        scroll: previousScroll,
        decision:
          'Not integrated: unexplained scrolling differences prompted narrowing to transcript/composer isolation only',
      }
    : null,
  workloads,
  primaryBaselineControl: control,
  limits: [
    'No paid/model calls; prior live budget unchanged',
    'Synthetic warm-host workload; first-process launch is one sample per arm, not cold OS or startup acceptance',
    'TaskDuration includes renderer work beyond JavaScript. The key-after-frame proxy is not INP or presentation feedback',
    'Private bytes sampled only twice per workload/arm, excluding shared working sets; JS heap follows ordinary GC',
    'Full transcript DOM preserved; no virtualization or initial-mount scaling improvement claimed',
    'No provider handshake/inference latency, full-release packaging/CI, non-Windows GUI or screen-reader certification',
  ],
};
await writeFile('docs/data/performance-0.8-results.json', JSON.stringify(result, null, 2) + '\n');
console.log(
  JSON.stringify(
    {
      primary: result.primary,
      rows: workloads.map((w) => ({
        size: w.size,
        kind: w.kind,
        openA: w.baseline.openMs.median,
        openB: w.candidate.openMs.median,
        typeA: w.baseline.typingTaskMs.median,
        typeB: w.candidate.typingTaskMs.median,
        streamA: w.baseline.streamTaskMs.median,
        streamB: w.candidate.streamTaskMs.median,
      })),
    },
    null,
    2,
  ),
);
