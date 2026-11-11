// Aggregate local correctness evidence; no raw project data or inference.
import { readFile, writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
const baseline = await readFile('.impeccable/graph-baseline.txt', 'utf8');
assert(baseline.includes('14 corpus failures:'));
const rust = await readFile('.impeccable/graph-tests.txt', 'utf8');
const relevant = [
  'codegraph::tests::roadmap_typescript_and_godot_import_corpus',
  'codegraph::tests::roadmap_comments_and_normalized_resource_paths',
  'codegraph::tests::roadmap_graph_refreshes_edited_imports_renames_and_deletions',
  'impact::tests::typed_and_godot_deleted_imports_keep_callers_in_the_handoff',
  'repomap::tests::old_import_extraction_cache_is_rebuilt_even_with_matching_file_metadata',
];
for (const name of relevant) assert(rust.includes(`test ${name} ... ok`));
assert(rust.includes('163 passed; 0 failed; 14 ignored'));
const native = JSON.parse(
  await readFile('.impeccable/graph-reliability/native-results.json', 'utf8'),
);
assert(native.passed && native.checks.length === 5);
const oracles = JSON.parse(
  await readFile('docs/data/graph-import-oracles-0.8-results.json', 'utf8'),
);
assert.equal(oracles.checks.length, 6);
assert(oracles.checks.every((c) => c.passed));
const report = {
  at: new Date().toISOString(),
  protocol: 'docs/graph-reliability-0.8.md',
  sourceBase: '9b83e9184523ea78d2e6648876ed5e76be88558c',
  coreCorpus: {
    expectations: 23,
    baselinePassed: 9,
    baselineFailed: 14,
    candidatePassed: 23,
    candidateFailed: 0,
    scope:
      '11 direct-import and reverse-impact positive/negative pairs plus one cross-language C# reference control',
  },
  additionalRustGates: relevant,
  rust: { passed: 163, failed: 0, ignored: 14 },
  native,
  oracles,
  cacheFormat: { before: 3, after: 4 },
  inferenceCalls: 0,
  previousLiveSlotsSpent: 13,
  limits: [
    'Approximate local graph, not complete language/compiler semantics.',
    'Full-line comments are excluded in TS/JS, C# and GDScript. Inline/block comments, multiline strings and dynamic expressions are not fully parsed.',
    'Arbitrary tsconfig paths, bundler overrides, package exports, Godot UID mapping and relative load/preload semantics are outside this checkpoint.',
    'Content refresh still uses file size/mtime; edits preserving both stamps and configuration-only graph changes are not certified.',
    'No universal graph accuracy, token saving, model-quality improvement or index-speed gain is inferred from this fixed corpus.',
    'Native validation is Windows debug/MCP with isolated fixtures; other desktop environments remain separate.',
  ],
};
await writeFile(
  'docs/data/graph-reliability-0.8-results.json',
  JSON.stringify(report, null, 2) + '\n',
);
console.log(
  JSON.stringify({
    core: report.coreCorpus,
    rust: report.rust,
    native: native.checks.length,
    oracles: oracles.checks.length,
    inferenceCalls: 0,
  }),
);
