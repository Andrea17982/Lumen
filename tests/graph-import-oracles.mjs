// Independent compiler/runtime semantics for the graph corpus. Isolated files, no inference.
import ts from 'typescript';
import { mkdir, mkdtemp, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { tmpdir } from 'node:os';
import { pathToFileURL } from 'node:url';
import assert from 'node:assert/strict';
import { command, godotDefault } from './quality-projects.mjs';

const root = await mkdtemp(path.join(tmpdir(), 'lumen-graph-oracle-'));
async function put(name, text) {
  const file = path.join(root, name);
  await mkdir(path.dirname(file), { recursive: true });
  await writeFile(file, text);
  return file;
}
for (const [name, text] of [
  ['package.json', '{"type":"module"}'],
  ['web/state.mts', 'export const count = 1;'],
  ['web/common.cts', 'export const count = 2;'],
  ['web/source.ts', 'export const count = 3;'],
  ['web/source.js', 'export const count = 4;'],
  ['web/view.jsx', 'export const count = 5;'],
  ['web/view-user.ts', "import {count} from './view.js';"],
  ['web/typed.mts', "import {count} from './state.mjs';"],
  ['web/common-user.cts', "import {count} from './common.cjs';"],
  ['web/typed-user.ts', "import {count} from './source.js';"],
  ['game/project.godot', 'config_version=5\n'],
  ['game/base.gd', 'extends Node\n'],
  ['game/scripts/derived.gd', "extends '../base.gd'\n"],
  ['game/scripts/quoted.gd', "extends Node\nconst value = preload('res://base.gd')\n"],
])
  await put(name, text);
const checks = [];
for (const [from, spec, expected] of [
  ['web/typed.mts', './state.mjs', 'web/state.mts'],
  ['web/common-user.cts', './common.cjs', 'web/common.cts'],
  ['web/typed-user.ts', './source.js', 'web/source.ts'],
  ['web/view-user.ts', './view.js', 'web/view.jsx'],
]) {
  const found = ts.resolveModuleName(
    spec,
    path.join(root, from),
    { moduleResolution: ts.ModuleResolutionKind.NodeNext, module: ts.ModuleKind.NodeNext },
    ts.sys,
  ).resolvedModule;
  assert(found);
  const actual = path.relative(root, found.resolvedFileName).replaceAll('\\', '/');
  assert.equal(actual, expected);
  checks.push({ oracle: 'TypeScript', from, spec, target: actual, passed: true });
}
const js = await import(pathToFileURL(path.join(root, 'web/source.js')).href);
assert.equal(js.count, 4);
await assert.rejects(import(pathToFileURL(path.join(root, 'web/state.mjs')).href), {
  code: 'ERR_MODULE_NOT_FOUND',
});
checks.push({
  oracle: 'Node.js',
  check: 'Explicit JavaScript resolves emitted JS and does not substitute a missing .mjs with .mts',
  passed: true,
});
const script = await put(
  'game/check.gd',
  `extends SceneTree
func _init():
    var derived = load('res://scripts/derived.gd')
    var quoted = load('res://scripts/quoted.gd')
    if derived == null or quoted == null:
        quit(1)
        return
    var instance = quoted.new()
    if derived.get_base_script().resource_path != 'res://base.gd' or instance.value.resource_path != 'res://base.gd':
        instance.free()
        quit(1)
        return
    instance.free()
    print('GRAPH_IMPORT_ORACLE_PASS')
    quit(0)
`,
);
const engine = process.env.LUMEN_GODOT_EXE || godotDefault;
const version = await command(engine, ['--version'], root);
assert.equal(version.code, 0);
const runtime = await command(
  engine,
  ['--headless', '--path', path.join(root, 'game'), '--script', script],
  root,
  30000,
);
assert.equal(runtime.code, 0, runtime.stderr);
assert(runtime.stdout.includes('GRAPH_IMPORT_ORACLE_PASS'), runtime.stdout + runtime.stderr);
checks.push({
  oracle: 'Godot',
  check:
    'Single-quoted res:// preload and script-relative inheritance resolve the expected base script',
  passed: true,
});
const report = {
  at: new Date().toISOString(),
  versions: { typescript: ts.version, node: process.version, godot: version.stdout.trim() },
  checks,
  inferenceCalls: 0,
  limits: [
    'Compiler/runtime checks validate fixture semantics; Rust graph conformance and mutation tests are separate.',
    'Not a full TypeScript module resolver, GDScript parser, C# compiler or whole-project semantic certification.',
  ],
};
await writeFile(
  'docs/data/graph-import-oracles-0.8-results.json',
  JSON.stringify(report, null, 2) + '\n',
);
console.log(JSON.stringify(report));
