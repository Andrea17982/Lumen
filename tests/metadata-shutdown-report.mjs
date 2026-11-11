import { readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import assert from 'node:assert/strict';
const root = '.impeccable/metadata-shutdown/';
const blocks = {};
for (const name of ['baseline-1', 'candidate-1', 'baseline-2', 'candidate-2']) {
  const value = JSON.parse(await readFile(root + name + '.json', 'utf8'));
  assert(value.complete && value.inferenceCalls === 0);
  assert.equal(value.samples.length, 40);
  assert(value.samples.every((s) => s.status === 'ok'));
  blocks[name] = value.samples;
}
const stats = (a) => {
  a = [...a].sort((a, b) => a - b);
  assert(a.length && a.every(Number.isFinite));
  return {
    samples: a.length,
    median: (a[Math.floor((a.length - 1) / 2)] + a[Math.ceil((a.length - 1) / 2)]) / 2,
    p95: a[Math.ceil(a.length * 0.95) - 1],
    min: a[0],
    max: a.at(-1),
  };
};
const summary = (rows) => ({
  versionMs: stats(rows.map((s) => s.versionMs)),
  catalogMs: stats(rows.map((s) => s.catalogMs)),
  cleanupMs: stats(
    rows.map((s) => {
      const v = Object.fromEntries(s.milestones);
      return v['cleanup-ended'] - v['query-ended'];
    }),
  ),
  spawnMs: stats(
    rows.map((s) => {
      const v = Object.fromEntries(s.milestones);
      return v['process-started'] - v['spawn-start'];
    }),
  ),
  processToInitializeAckMs: stats(
    rows.map((s) => {
      const v = Object.fromEntries(s.milestones);
      return v['initialize-acknowledged'] - v['process-started'];
    }),
  ),
  gracefulRootExit: rows.filter((s) => s.milestones.some(([n]) => n === 'graceful-root-exit'))
    .length,
  modelCounts: [...new Set(rows.map((s) => s.modelCount))],
});
const providers = [];
for (const provider of ['claude', 'codex']) {
  const a = Object.entries(blocks)
    .filter(([n]) => n.startsWith('baseline'))
    .flatMap(([, s]) => s)
    .filter((s) => s.provider === provider);
  const b = Object.entries(blocks)
    .filter(([n]) => n.startsWith('candidate'))
    .flatMap(([, s]) => s)
    .filter((s) => s.provider === provider);
  assert.equal(a.length, 40);
  assert.equal(b.length, 40);
  assert.deepEqual([...new Set(a.map((s) => s.version))], [...new Set(b.map((s) => s.version))]);
  const baseline = summary(a),
    candidate = summary(b);
  const reduction = 1 - candidate.catalogMs.median / baseline.catalogMs.median;
  assert.deepEqual(baseline.modelCounts, candidate.modelCounts);
  assert(reduction >= 0.2, 'Both complete metadata call medians must improve at least 20%');
  providers.push({
    provider,
    version: a[0].version,
    baseline,
    candidate,
    reduction,
    blocks: Object.fromEntries(
      Object.entries(blocks).map(([n, s]) => [
        n,
        summary(s.filter((s) => s.provider === provider)),
      ]),
    ),
  });
}
const hash = async (name) =>
  createHash('sha256')
    .update(await readFile(root + name))
    .digest('hex');
const report = {
  at: new Date().toISOString(),
  protocol: 'docs/metadata-shutdown-0.8.md',
  order: ['baseline-1', 'candidate-1', 'baseline-2', 'candidate-2'],
  metadataSamples: 160,
  versionSamples: 160,
  driver: 'Windows debug Rust test drivers; actual installed CLIs; isolated configuration',
  hashes: {
    baseline: await hash('baseline-test.exe'),
    candidate: await hash('candidate-test.exe'),
  },
  providers,
  inferenceCalls: 0,
  previousLiveSlotsSpent: 13,
  limits: [
    'Fresh CLI processes in warm OS/profile history, not cold startup or model TTFT.',
    'Only metadata shutdown changes; model-turn and non-Windows termination paths remain unchanged.',
    'Added suspended assignment/job tracking is included in spawn and total times; absolute times remain host-dependent.',
  ],
};
try {
  const final = JSON.parse(await readFile(root + 'final-confirmation.json', 'utf8'));
  assert(
    final.complete && final.samples.length === 40 && final.samples.every((s) => s.status === 'ok'),
  );
  report.finalConfirmation = {
    hash: await hash('final-test.exe'),
    providers: providers.map((p) => {
      const measured = summary(final.samples.filter((s) => s.provider === p.provider));
      const reduction = 1 - measured.catalogMs.median / p.baseline.catalogMs.median;
      assert(reduction >= 0.2);
      return { provider: p.provider, measured, reduction };
    }),
  };
} catch (error) {
  if (error.code !== 'ENOENT') throw error;
}
await writeFile(
  'docs/data/metadata-shutdown-0.8-results.json',
  JSON.stringify(report, null, 2) + '\n',
);
console.log(
  JSON.stringify(
    providers.map((p) => ({
      provider: p.provider,
      baseline: p.baseline.catalogMs.median,
      candidate: p.candidate.catalogMs.median,
      reduction: p.reduction,
      cleanupBaseline: p.baseline.cleanupMs.median,
      cleanupCandidate: p.candidate.cleanupMs.median,
      spawnBaseline: p.baseline.spawnMs.median,
      spawnCandidate: p.candidate.spawnMs.median,
      graceful: p.candidate.gracefulRootExit,
    })),
  ),
);
