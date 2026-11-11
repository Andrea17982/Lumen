// Sanitized attribution, keeping actual installed CLIs separate from offline provider turns.
import { readFile, writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
import path from 'node:path';
const root = path.resolve(process.env.LUMEN_TURN_LATENCY_ROOT || '.impeccable/turn-latency') + '/';
const offline = JSON.parse(await readFile(root + 'offline-results.json', 'utf8'));
const installed = JSON.parse(await readFile(root + 'cli-raw.json', 'utf8'));
assert(offline.completedAt && installed.complete);
assert.equal(offline.samples.length, 80);
assert.equal(offline.build, 'isolated-release');
const stats = (values) => {
  const a = values.filter(Number.isFinite).sort((a, b) => a - b);
  return {
    samples: a.length,
    median: a.length
      ? (a[Math.floor((a.length - 1) / 2)] + a[Math.ceil((a.length - 1) / 2)]) / 2
      : null,
    p95: a.length >= 20 ? a[Math.ceil(a.length * 0.95) - 1] : null,
    min: a[0] ?? null,
    max: a.at(-1) ?? null,
  };
};
const spans = (rows, field = 'timings') => {
  const definitions = {
    stageCatalogMs: ['catalog', 'capacity'],
    capacityAndDispatchMs: ['capacity', 'starting-provider'],
    sharedSetupMs: ['starting-provider', 'adapter-start'],
    adapterSetupMs: ['adapter-start', 'spawn-start'],
    spawnMs: ['spawn-start', 'process-started'],
    processToProtocolReadyMs: ['process-started', 'protocol-ready'],
    protocolToEnqueueMs: ['protocol-ready', 'request-enqueued'],
    enqueueToObservedTextMs: ['request-enqueued', 'first-text'],
    stageTotalMs: ['catalog', 'return'],
    schemaMs: ['version-ready', 'schema-ready'],
  };
  return Object.fromEntries(
    Object.entries(definitions).map(([label, [from, to]]) => [
      label,
      stats(
        rows.map((s) => {
          const v = Object.fromEntries(s[field]);
          return v[to] !== undefined && v[from] !== undefined ? v[to] - v[from] : NaN;
        }),
      ),
    ]),
  );
};
const workloads = [];
for (const provider of ['claude', 'gpt'])
  for (const mode of ['initial', 'resume']) {
    const rows = offline.samples.filter((s) => s.provider === provider && s.mode === mode);
    assert.equal(rows.length, 20);
    workloads.push({
      provider,
      mode,
      samples: 20,
      firstProcess: rows.find((s) => s.firstProcessForAccount) || null,
      acceptanceMs: stats(rows.map((s) => s.acceptanceMs)),
      preparationMs: stats(rows.map((s) => s.preparationMs)),
      completionObservedMs: stats(rows.map((s) => s.completedMs)),
      spans: spans(rows),
    });
  }
const cli = [];
for (const provider of ['claude', 'codex']) {
  const rows = installed.samples.filter((s) => s.provider === provider);
  assert(rows.length > 0);
  const valid = rows.filter((s) => s.status === 'ok');
  const versions = [
    ...new Set(
      rows.map((s) => s.version?.match(/\d+\.\d+\.\d+(?:[-+][\w.-]+)?/)?.[0]).filter(Boolean),
    ),
  ];
  const timings = {
    versionMs: stats(rows.map((s) => s.versionMs)),
    catalogMs: stats(valid.map((s) => s.catalogMs)),
  };
  for (const [name, from, to] of [
    ['spawnMs', 'spawn-start', 'process-started'],
    ['processToInitializeAckMs', 'process-started', 'initialize-acknowledged'],
    ['metadataAfterInitMs', 'initialize-acknowledged', 'query-ended'],
    ['cleanupMs', 'query-ended', 'cleanup-ended'],
  ])
    timings[name] = stats(
      valid.map((s) => {
        const v = Object.fromEntries(s.milestones);
        return v[to] - v[from];
      }),
    );
  cli.push({
    provider,
    versions,
    attempted: rows.length,
    successful: valid.length,
    status: valid.length === 20 ? 'complete' : 'prerequisite-unavailable',
    firstProcess: {
      versionMs: rows[0].versionMs,
      catalogMs: rows[0].catalogMs,
      status: rows[0].status,
      modelCount: rows[0].modelCount,
      milestones: rows[0].milestones,
    },
    timings,
  });
}
const report = {
  at: new Date().toISOString(),
  protocol: 'docs/turn-latency-0.8.md',
  offline: { build: offline.build, sha256: offline.sha256, samples: 80, workloads },
  installedCli: {
    driver:
      'Rust debug test driver, actual installed CLI processes; isolated configuration, no copied authentication',
    providers: cli,
  },
  limits: [
    'No provider inference/TTFT or official-app parity measurement. Metadata readiness is not model readiness.',
    'Stage phase timings include local bookkeeping. First normalized text includes scheduling/buffering (up to the existing 24 ms batch), not a network first byte.',
    'Source-stamped transport milestones use a process-wide monotonic origin; normalized event/text stamps are controller observations. Stored values are relative offsets only.',
    'Command preparation covers accepted direct submissions. Queue/context before a stage, rejected submissions, Auto classification inference, Orchestra scheduling and provider/network/tool latency are not independently attributed here.',
    'Completion observed by native IPC polling includes up to a 25 ms polling interval plus IPC; it is distinct from stage return.',
    'Initial samples are fresh native sessions, not cold OS/app launches. Metadata/account caches are warm after the first sample; existing catalog TTL is 300 seconds.',
    'No warm process pool or general speedup is implemented or inferred. Independent CLI invocations retain config/account isolation.',
  ],
  inferenceCalls: 0,
  previousLiveSlotsSpent: 13,
};
await writeFile('docs/data/turn-latency-0.8-results.json', JSON.stringify(report, null, 2) + '\n');
console.log(
  JSON.stringify({
    workloads: workloads.map((w) => ({
      provider: w.provider,
      mode: w.mode,
      preparationMs: w.preparationMs.median,
      completedMs: w.completionObservedMs.median,
      handshakeMs: w.spans.processToProtocolReadyMs.median,
    })),
    installedCli: cli.map((c) => ({
      provider: c.provider,
      attempted: c.attempted,
      status: c.status,
      timings: c.timings,
    })),
  }),
);
