// Opt-in protocol probe: installed Codex CLI, fresh unauthenticated profile, local MCP only.
// No login, turn/start or model inference. Build the debug binary before running.
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, mkdir, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const root = await mkdtemp(path.join(os.tmpdir(), 'lumen-installed-mcp-'));
const profile = path.join(root, 'profile');
const workspace = path.join(root, 'workspace');
await Promise.all([mkdir(profile), mkdir(workspace)]);
await writeFile(
  path.join(profile, 'config.toml'),
  `cli_auth_credentials_store = "file"
[mcp_servers.local_fixture]
command = ${JSON.stringify(process.execPath)}
args = [${JSON.stringify(path.join(repo, 'tests/fixtures/shared-mcp.mjs'))}]
[mcp_servers.local_fixture.env]
OWNER_MARKER = "installed-cli-offline"
STARTUP_DELAY_MS = "2000"
`,
);
const env = { ...process.env, LUMEN_TEST_DATA_DIR: root };
delete env.LUMEN_BROWSER_URL;
delete env.LUMEN_BROWSER_CAP;
const child = spawn(
  path.join(repo, 'src-tauri/target/debug', process.platform === 'win32' ? 'lumen.exe' : 'lumen'),
  ['--lumen-native-account-mcp', profile, workspace, 'full'],
  { env, windowsHide: true, stdio: ['pipe', 'pipe', 'ignore'] },
);
const exited = once(child, 'exit');
const lines = createInterface({ input: child.stdout })[Symbol.asyncIterator]();
let id = 0;
async function rpc(method, params = {}) {
  const current = ++id;
  child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id: current, method, params }) + '\n');
  let timer;
  try {
    return await Promise.race([
      (async () => {
        for (;;) {
          const row = await lines.next();
          assert(!row.done, `Gateway closed during ${method}`);
          const message = JSON.parse(row.value);
          assert.equal(message.id, current, 'Unexpected server request without consent');
          assert(!message.error, JSON.stringify(message.error));
          return message.result;
        }
      })(),
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(new Error(`Timeout during ${method}`)), 45000);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}
try {
  await rpc('initialize', {
    protocolVersion: '2025-06-18',
    capabilities: {},
    clientInfo: { name: 'lumen-offline-probe', version: '1' },
  });
  const started = performance.now();
  const catalog = await rpc('tools/list');
  const metadataMs = Math.round(performance.now() - started);
  const tool = catalog.tools.find((entry) => entry.description?.includes('local_fixture/echo:'));
  assert(tool, 'Initial catalog omitted the delayed local MCP service');
  const result = await rpc('tools/call', {
    name: tool.name,
    arguments: { count: 3, enabled: false },
  });
  assert.deepEqual(JSON.parse(result.content[0].text), {
    owner: 'installed-cli-offline',
    arguments: { count: 3, enabled: false },
  });
  const report = {
    passed: true,
    metadataMs,
    delayedStartupMs: 2000,
    inferenceTurns: 0,
    login: false,
  };
  await writeFile(path.join(root, 'result.json'), JSON.stringify(report, null, 2));
  console.log(JSON.stringify({ ...report, evidence: root }));
} finally {
  child.stdin.end();
  let timer;
  try {
    await Promise.race([
      exited,
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(new Error('Gateway cleanup timed out')), 10000);
      }),
    ]);
  } catch {
    if (process.platform === 'win32')
      spawnSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], {
        windowsHide: true,
        stdio: 'ignore',
      });
    else child.kill('SIGKILL');
    throw new Error(`Gateway cleanup failed; evidence: ${root}`);
  } finally {
    clearTimeout(timer);
  }
}
