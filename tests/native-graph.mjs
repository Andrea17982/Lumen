// Exercise the actual native graph MCP, retaining a process across edits and then reloading disk cache.
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { mkdtemp, mkdir, writeFile, rename, unlink, readdir, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
const base = await mkdtemp(path.join(tmpdir(), 'lumen-native-graph-'));
const root = path.join(base, 'project'),
  data = path.join(base, 'data');
await mkdir(data, { recursive: true });
async function put(name, text) {
  const file = path.join(root, name);
  await mkdir(path.dirname(file), { recursive: true });
  await writeFile(file, text);
}
for (const [name, text] of [
  ['web/state.mts', 'export const count = 1;\n'],
  ['web/use.test.mts', "import {count as x} from './state.mjs';\nexport const value = x;\n"],
  ['game/project.godot', 'config_version=5\n'],
  ['game/base.gd', 'extends Node\n'],
  ['game/scripts/derived.gd', "extends '../base.gd'\n"],
])
  await put(name, text);
async function connect() {
  const child = spawn(
    path.resolve('src-tauri/target/debug/lumen.exe'),
    ['--lumen-graph-mcp', data, root],
    { windowsHide: true, stdio: 'pipe' },
  );
  child.stderr.resume();
  let next = 0;
  const pending = new Map();
  const closed = new Promise((resolve) => child.once('close', resolve));
  createInterface({ input: child.stdout }).on('line', (line) => {
    const value = JSON.parse(line);
    const entry = pending.get(value.id);
    if (entry) {
      pending.delete(value.id);
      clearTimeout(entry.timer);
      entry.resolve(value);
    }
  });
  function request(method, params) {
    return new Promise((resolve, reject) => {
      const id = ++next;
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(Error('Native graph RPC timed out'));
        child.kill();
      }, 15000);
      pending.set(id, { resolve, reject, timer });
      child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n');
    });
  }
  child.on('error', (error) => {
    for (const p of pending.values()) {
      clearTimeout(p.timer);
      p.reject(error);
    }
    pending.clear();
  });
  const init = await request('initialize', { protocolVersion: '2025-06-18' });
  assert.equal(init.result.serverInfo.name, 'lumen_graph');
  return {
    async tool(name, args) {
      const value = await request('tools/call', { name, arguments: args });
      assert(!value.error && !value.result.isError, JSON.stringify(value));
      return value.result.content.map((c) => c.text || '').join('\n');
    },
    async close() {
      child.stdin.end();
      const timer = setTimeout(() => child.kill(), 5000);
      await closed;
      clearTimeout(timer);
    },
  };
}
const checks = [];
let client = await connect();
try {
  let impact = await client.tool('impact', {
    targets: ['web/state.mts', 'game/base.gd'],
    depth: 1,
  });
  assert(impact.includes('[1] web/use.test.mts') && impact.includes('[1] game/scripts/derived.gd'));
  assert(impact.includes('Tests to run: web/use.test.mts'));
  checks.push('Native MCP exposes typed-module and relative-Godot callers and affected tests');
  const symbol = await client.tool('symbol', { name: 'count', file: 'web/state.mts', body: true });
  assert(/web\/state\.mts:1(?:\s|$)/.test(symbol) && symbol.includes('export const count'), symbol);
  checks.push('Native symbol lookup preserves exact declaration location and body');
  await rename(path.join(root, 'web/state.mts'), path.join(root, 'web/renamed.mts'));
  await rename(path.join(root, 'game/base.gd'), path.join(root, 'game/renamed_base.gd'));
  await put(
    'web/use.test.mts',
    "import {count as x} from './renamed.mjs';\nexport const value = x;\n",
  );
  await put('game/scripts/derived.gd', "extends '../renamed_base.gd'\n");
  impact = await client.tool('impact', {
    targets: ['web/renamed.mts', 'game/renamed_base.gd'],
    depth: 1,
  });
  assert(impact.includes('[1] web/use.test.mts') && impact.includes('[1] game/scripts/derived.gd'));
  const old = await client.tool('impact', { targets: ['web/state.mts'], depth: 1 });
  assert(old.includes('neither an indexed file nor a known symbol'));
  checks.push(
    'One running native process refreshes edited imports and renamed targets without stale paths',
  );
  await client.close();
  client = await connect();
  impact = await client.tool('impact', { targets: ['web/renamed.mts'], depth: 1 });
  assert(impact.includes('[1] web/use.test.mts'));
  const files = await readdir(path.join(data, 'repomap'));
  assert(files.length > 0);
  const index = JSON.parse(await readFile(path.join(data, 'repomap', files[0]), 'utf8'));
  assert.equal(index.version, 4);
  assert(!index.files['web/state.mts']);
  checks.push('A restarted native process reads the refreshed versioned disk index');
  await unlink(path.join(root, 'web/renamed.mts'));
  const deleted = await client.tool('impact', { targets: ['web/renamed.mts'], depth: 1 });
  assert(deleted.includes('neither an indexed file nor a known symbol'));
  checks.push('Deleted targets stop being reported as indexed definitions');
} finally {
  await client.close();
}
const report = {
  at: new Date().toISOString(),
  build: 'Windows debug native graph MCP; isolated temporary project/data',
  passed: true,
  checks,
  inferenceCalls: 0,
};
await mkdir('.impeccable/graph-reliability', { recursive: true });
await writeFile(
  '.impeccable/graph-reliability/native-results.json',
  JSON.stringify(report, null, 2),
);
console.log(JSON.stringify(report));
