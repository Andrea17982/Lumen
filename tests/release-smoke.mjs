// Runs the installed release helper on a clean packaging runner; no model/credentials needed.
import { spawnSync } from 'node:child_process';
import { mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
const executable = process.env.LUMEN_RELEASE_EXE;
if (!executable)
  throw Error('Set LUMEN_RELEASE_EXE to the installed or portable release executable.');
const root = await mkdtemp(path.join(tmpdir(), 'lumen-release-probe-'));
const requests = [
  { jsonrpc: '2.0', id: 1, method: 'initialize', params: { protocolVersion: '2025-06-18' } },
  { jsonrpc: '2.0', id: 2, method: 'tools/list' },
];
const result = spawnSync(executable, ['--lumen-graph-mcp', root, root], {
  input: requests.map((r) => JSON.stringify(r)).join('\n') + '\n',
  encoding: 'utf8',
  windowsHide: true,
  timeout: 15000,
});
assert.equal(result.status, 0, String(result.error || result.stderr));
const replies = result.stdout
  .trim()
  .split(/\r?\n/)
  .map((line) => JSON.parse(line));
assert.equal(replies[0].result.serverInfo.name, 'lumen_graph');
assert.equal(replies[1].result.tools.length, 6);
console.log('Installed release MCP initialization and tool discovery passed.');
