// Runs the installed release helper on a clean packaging runner; no model/credentials needed.
import { spawnSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdtemp, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import assert from 'node:assert/strict';
const executable = process.env.LUMEN_RELEASE_EXE;
const expectedVersion = JSON.parse(
  await readFile(new URL('../package.json', import.meta.url), 'utf8'),
).version;
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
assert.equal(replies[0].result.serverInfo.version, expectedVersion);
assert.equal(replies[1].result.tools.length, 6);
const browser = spawnSync(executable, ['--lumen-browser-mcp'], {
  input: requests.map((r) => JSON.stringify(r)).join('\n') + '\n',
  env: {
    ...process.env,
    LUMEN_BROWSER_URL: 'http://127.0.0.1:9/call',
    LUMEN_BROWSER_CAP: 'discovery-only',
  },
  encoding: 'utf8',
  windowsHide: true,
  timeout: 15000,
});
assert.equal(browser.status, 0, String(browser.error || browser.stderr));
const browserReplies = browser.stdout
  .trim()
  .split(/\r?\n/)
  .map((line) => JSON.parse(line));
assert.equal(browserReplies[0].result.serverInfo.name, 'lumen_browser');
assert.equal(browserReplies[0].result.serverInfo.version, expectedVersion);
assert.equal(browserReplies[1].result.tools.length, 8);
const moli = path.join(path.dirname(executable), 'resources/moli/win32-x64/moli.exe');
assert(existsSync(moli), `Bundled browser missing: ${moli}`);
const version = spawnSync(moli, ['--version'], {
  encoding: 'utf8',
  windowsHide: true,
  timeout: 15000,
});
assert.equal(version.status, 0, String(version.error || version.stderr));
assert.match(version.stdout, /moli 1\.1\.23/);
console.log('Installed release graph/browser MCP discovery and bundled Moli executable passed.');
