// Simulates a calling CLI with no native elicitation UI. Consent must come from Lumen's host.
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { once } from 'node:events';
import assert from 'node:assert/strict';
const [executable, profile, workspace] = process.argv.slice(2);
const child = spawn(executable, ['--lumen-native-account-mcp', profile, workspace, 'full'], {
  windowsHide: true,
  stdio: 'pipe',
  env: process.env,
});
const exited = once(child, 'exit');
const iterator = createInterface({ input: child.stdout })[Symbol.asyncIterator]();
let id = 0;
async function rpc(method, params) {
  child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id: ++id, method, params }) + '\n');
  const row = await iterator.next();
  assert(!row.done);
  const message = JSON.parse(row.value);
  assert(!message.method, 'Native caller cannot handle consent: host must display it');
  assert.equal(message.id, id);
  assert(!message.error, JSON.stringify(message));
  return message.result;
}
try {
  await rpc('initialize', {
    protocolVersion: '2025-06-18',
    capabilities: {},
    clientInfo: { name: 'offline-caller', version: '1' },
  });
  const list = await rpc('tools/list', {});
  const read = list.tools.find((tool) => tool.description.includes('/read:'));
  const result = await rpc('tools/call', {
    name: read.name,
    arguments: { elicit: true, count: 11 },
  });
  assert.equal(JSON.parse(result.content[0].text).arguments.count, 11);
} finally {
  child.stdin.end();
  await exited;
}
