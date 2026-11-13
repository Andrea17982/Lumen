// Aggregate controlled payload evidence, never provider logs or private project contents.
import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';

const directory = '.impeccable/context-budget/';
const baseline = await readFile(`${directory}baseline.txt`, 'utf8');
const candidate = await readFile(`${directory}candidate.txt`, 'utf8');
const metric = (text, name) => {
  const line = text.split(/\r?\n/).find((row) => row.startsWith(`${name} `));
  assert(line, `Missing ${name}`);
  return JSON.parse(line.slice(name.length + 1));
};
assert(baseline.includes('0 passed; 5 failed'));
assert(candidate.includes('180 passed; 0 failed; 14 ignored'));
for (const name of [
  'codegraph::tests::roadmap_typescript_and_godot_import_corpus',
  'codegraph::tests::roadmap_comments_and_normalized_resource_paths',
  'codegraph::tests::roadmap_graph_refreshes_edited_imports_renames_and_deletions',
  'impact::tests::typed_and_godot_deleted_imports_keep_callers_in_the_handoff',
])
  assert(candidate.includes(`test ${name} ... ok`));
const before = metric(baseline, 'CONTEXT_PROMPT_METRICS');
const after = metric(candidate, 'CONTEXT_PROMPT_METRICS');
const paired = (left, right) => ({
  baseline: left,
  candidate: right,
  bytesChangePercent: Number(((right.bytes / left.bytes - 1) * 100).toFixed(1)),
  localTokensChangePercent: Number(((right.localTokens / left.localTokens - 1) * 100).toFixed(1)),
});
const corpus = after.map((row) => {
  const old = before.find((item) => item.case === row.case);
  assert.equal(row.requirementCopies, 2);
  assert.equal(old.requirementCopies, 4);
  assert.equal(row.attachmentCopies, 1);
  assert.equal(old.attachmentCopies, 2);
  assert.equal(row.fullStateRequirements, old.fullStateRequirements);
  return { case: row.case, ...paired(old, row) };
});
const nativeBefore = JSON.parse(await readFile(`${directory}native-baseline.json`, 'utf8'));
const nativeAfter = JSON.parse(await readFile(`${directory}native-candidate.json`, 'utf8'));
assert.equal(nativeAfter.checks.length, 5);
assert.equal(nativeAfter.samples.length, 6);
const native = nativeAfter.samples.map((row) => ({
  case: row.case,
  ...paired(
    nativeBefore.samples.find((item) => item.case === row.case),
    row,
  ),
}));
const attachmentBefore = metric(baseline, 'CONTEXT_ATTACHMENT_METRICS');
const attachmentAfter = metric(candidate, 'CONTEXT_ATTACHMENT_METRICS');
assert.equal(attachmentAfter.previewContentBytes, 49152);
assert.equal(attachmentBefore.previewContentBytes, 81920);
const graph = JSON.parse(
  await readFile('.impeccable/graph-reliability/native-results.json', 'utf8'),
);
const quality = JSON.parse(
  await readFile('.impeccable/review/native-quality-results.json', 'utf8'),
);
const orchestra = JSON.parse(
  await readFile('.impeccable/review/native-orchestra-continuity-results.json', 'utf8'),
);
assert(quality.passed && quality.checks.length === 12);
assert(orchestra.checks.length === 4);
assert(graph.passed && graph.checks.length === 5);
const report = {
  date: '2026-11-12',
  sourceBase: '250e6ec',
  protocol: 'docs/context-graph-budget-0.8.md',
  inferenceCalls: 0,
  historicalLiveSlotsSpent: 13,
  rust: { passed: 180, failed: 0, ignored: 14, newRegressions: 5, baselineRegressionsFailed: 5 },
  corpus,
  attachments: { baseline: attachmentBefore, candidate: attachmentAfter },
  native: { samples: native, checks: nativeAfter.checks },
  regressionChecks: {
    quality: quality.checks.length,
    orchestra: orchestra.checks.length,
    graphMcp: graph.checks.length,
    graphCorpusExpectations: 23,
  },
  limits: [
    'Raw inputs use different temporary UUIDs and paths; local o200k counts can vary slightly between runs.',
    'Fixed fixtures and payload delivery, not provider-billed tokens, latency or code-quality comparisons.',
    'Updated project notes deliberately grow versus the old clipped payload to retain middle requirements; unchanged native turns remain incremental.',
    '48 KiB caps inline/preview attachment content bytes, not path/envelope bytes or the complete prompt.',
    'No global model-context allocation, output reservation or native context-window guarantee is added.',
    'Full durable task facts stay available; ordinary transcript excerpts and approximate graph limitations remain.',
    'Compact metadata tuples, parser replacement and end-to-end model comprehension remain unproven; native desktop proof is Windows-only.',
  ],
};
await writeFile(
  'docs/data/context-graph-budget-0.8-results.json',
  `${JSON.stringify(report, null, 2)}\n`,
);
console.log(
  JSON.stringify(
    { corpus, attachments: report.attachments, native, regressionChecks: report.regressionChecks },
    null,
    2,
  ),
);
