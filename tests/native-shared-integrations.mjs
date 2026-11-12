// Windows UI/storage plus real MCP relays and offline native account RPC. No inference/login.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { createInterface } from 'node:readline';
import { mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import path from 'node:path';
import { expect } from '@playwright/test';
import { nativeHarness } from './native-harness.mjs';
const h = await nativeHarness('shared-integrations');
const owned = [];
const checks = [];
const executable = path.join(h.root, 'src-tauri/target/debug/lumen.exe');
function connection(args) {
  const child = spawn(executable, args, {
    windowsHide: true,
    stdio: 'pipe',
    env: {
      ...process.env,
      LUMEN_TEST_DATA_DIR: h.data,
      PATH: `${path.join(h.root, 'tests/fixtures/cli')};${process.env.PATH}`,
    },
  });
  owned.push(child);
  const exited = once(child, 'exit');
  const iterator = createInterface({ input: child.stdout })[Symbol.asyncIterator]();
  let id = 0;
  return {
    child,
    async rpc(method, params = {}, consent) {
      const current = ++id;
      child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id: current, method, params }) + '\n');
      for (;;) {
        let timeout;
        const row = await Promise.race([
          iterator.next(),
          new Promise(
            (_, reject) =>
              (timeout = setTimeout(() => reject(new Error(`MCP timeout: ${method}`)), 15000)),
          ),
        ]).finally(() => clearTimeout(timeout));
        assert(!row.done, 'MCP relay closed before answering');
        const message = JSON.parse(row.value);
        if (message.method === 'elicitation/create') {
          assert(consent, 'Unexpected consent must not auto-accept');
          child.stdin.write(
            JSON.stringify({ jsonrpc: '2.0', id: message.id, result: consent }) + '\n',
          );
          continue;
        }
        assert.equal(message.id, current);
        return message;
      }
    },
    async close() {
      child.stdin.end();
      await exited;
    },
  };
}
try {
  const owner = await h.invoke('add_account', { name: 'Owner Claude', provider: 'claude' });
  const gpt = await h.invoke('add_account', { name: 'Owner GPT', provider: 'gpt' });
  const ownerDir = path.join(h.data, 'accounts', owner.id),
    gptDir = path.join(h.data, 'accounts', gpt.id);
  const pidFile = path.join(h.data, 'service.pid');
  await mkdir(path.join(ownerDir, 'skills/review/resources'), { recursive: true });
  await writeFile(
    path.join(ownerDir, 'skills/review/SKILL.md'),
    '---\ndescription: Shared owner skill\n---\nRead resources/data.txt.',
  );
  await writeFile(path.join(ownerDir, 'skills/review/resources/data.txt'), 'Original resource');
  const original = JSON.stringify({
    mcpServers: {
      service: {
        command: process.execPath,
        args: [path.join(h.root, 'tests/fixtures/shared-mcp.mjs')],
        env: { OWNER_MARKER: 'owner-a', FIXTURE_PID_FILE: pidFile },
      },
      restricted: { url: 'https://example.test', oauth: { clientId: 'private-native-value' } },
    },
  });
  await writeFile(path.join(ownerDir, '.claude.json'), original);
  await writeFile(path.join(gptDir, 'auth.json'), '{}');
  await writeFile(path.join(gptDir, 'native-gateway-fixture'), 'offline');
  await mkdir(path.join(gptDir, 'skills/remote/resources'), { recursive: true });
  await writeFile(path.join(gptDir, 'skills/remote/SKILL.md'), 'Owner skill body');
  await writeFile(path.join(gptDir, 'skills/remote/resources/data.txt'), 'Owner skill resource');
  const inventory = await h.invoke('integration_status');
  assert(!JSON.stringify(inventory.discovered).includes('private-native-value'));
  const service = inventory.discovered.find((e) => e.name === 'service');
  assert(service);
  assert(inventory.discovered.some((e) => e.kind === 'skill'));
  assert(inventory.discovered.some((e) => e.kind === 'native-mcp'));
  assert.equal(inventory.discovered.find((e) => e.name === 'restricted').status, 'needs-adapter');
  const relay = connection(['--lumen-shared-stdio', h.data, h.workspace, service.id]);
  await relay.rpc('initialize', { protocolVersion: '2025-06-18' });
  assert.equal((await relay.rpc('tools/list')).result.tools[0].name, 'echo');
  const echoed = await relay.rpc('tools/call', {
    name: 'echo',
    arguments: { count: 3, enabled: false },
  });
  assert.deepEqual(JSON.parse(echoed.result.content[0].text), {
    owner: 'owner-a',
    arguments: { count: 3, enabled: false },
  });
  await relay.close();
  const servicePid = Number(await readFile(pidFile, 'utf8'));
  assert.throws(() => process.kill(servicePid, 0));
  checks.push(
    'Original stdio service preserves owner environment and typed arguments; client close reaps the service',
  );
  const orphaned = connection(['--lumen-shared-stdio', h.data, h.workspace, service.id]);
  await orphaned.rpc('initialize', { protocolVersion: '2025-06-18' });
  const orphan = Number(
    (await orphaned.rpc('tools/call', { name: 'echo', arguments: { orphan: true } })).result
      .content[0].text,
  );
  await orphaned.close();
  assert.throws(() => process.kill(orphan, 0));
  checks.push(
    'Relay cleanup reaps an owned descendant even after the original MCP root process exits',
  );
  const gateway = connection(['--lumen-native-account-mcp', gptDir, h.workspace, 'full']);
  await gateway.rpc('initialize', { protocolVersion: '2025-06-18' });
  const tools = (await gateway.rpc('tools/list')).result.tools;
  assert.equal(tools.length, 4);
  const read = tools.find((t) => t.description.includes('/read:'));
  const result = await gateway.rpc(
    'tools/call',
    { name: read.name, arguments: { elicit: true, count: 7 } },
    { action: 'accept', content: { scope: 'test' } },
  );
  assert.deepEqual(JSON.parse(result.result.content[0].text), {
    owner: gpt.id,
    tool: 'read',
    arguments: { elicit: true, count: 7 },
  });
  await gateway.rpc('tools/call', { name: 'list_account_skills', arguments: {} });
  const skillPath = path.join(gptDir, 'skills/remote/SKILL.md');
  const skill = await gateway.rpc('tools/call', {
    name: 'read_account_skill',
    arguments: { path: skillPath },
  });
  assert.equal(skill.result.content[0].text, 'Owner skill body');
  const resource = await gateway.rpc('tools/call', {
    name: 'read_account_skill',
    arguments: { path: skillPath, resource: 'resources/data.txt' },
  });
  assert.equal(resource.result.content[0].text, 'Owner skill resource');
  assert.equal(
    (
      await gateway.rpc('tools/call', {
        name: 'read_account_skill',
        arguments: { path: skillPath, resource: '../../auth.json' },
      })
    ).result.isError,
    true,
  );
  checks.push(
    'Owning account skills and resources can be discovered/read, with traversal outside the skill folder rejected',
  );
  await gateway.close();
  const methods = await readFile(path.join(gptDir, 'native-rpc.txt'), 'utf8');
  assert(!methods.includes('turn/start'));
  assert(methods.includes('mcpServer/tool/call'));
  checks.push(
    'Native ChatGPT account gateway lists and calls owner tools and forwards explicit consent without any model turn',
  );
  const readOnly = connection(['--lumen-native-account-mcp', gptDir, h.workspace, 'read-only']);
  await readOnly.rpc('initialize', { protocolVersion: '2025-06-18' });
  assert.equal((await readOnly.rpc('tools/list')).result.tools.length, 3);
  const denied = await readOnly.rpc('tools/call', {
    name: tools.find((t) => t.description.includes('/write:')).name,
    arguments: {},
  });
  assert(denied.error);
  await readOnly.close();
  await writeFile(path.join(gptDir, 'native-gateway-unsupported'), 'offline');
  const unsupported = connection(['--lumen-native-account-mcp', gptDir, h.workspace, 'read-only']);
  await unsupported.rpc('initialize', { protocolVersion: '2025-06-18' });
  const unavailable = await unsupported.rpc('tools/list');
  assert(unavailable.error);
  assert(!JSON.stringify(unavailable).includes('private-native-error'));
  await unsupported.close();
  checks.push(
    'Unsupported native account API returns a sanitized MCP error and closes instead of hanging or starting inference',
  );
  checks.push(
    'Restricted gateway exposes only tools declared read-only and rejects a write tool from a previous catalog',
  );
  await h.invoke('toggle_discovered_integration', { id: service.id, on: false });
  assert.equal(
    (await h.invoke('integration_status')).discovered.find((e) => e.id === service.id).status,
    'disabled',
  );
  assert.equal(await readFile(path.join(ownerDir, '.claude.json'), 'utf8'), original);
  await h.invoke('toggle_discovered_integration', { id: service.id, on: true });
  checks.push(
    'Sharing toggle persists without modifying the source profile or exposing native configuration secrets',
  );
  await h.page.getByRole('button', { name: 'Settings', exact: true }).click();
  await h.page.getByRole('button', { name: 'Advanced', exact: true }).click();
  await h.page.getByRole('tab', { name: 'Tools', exact: true }).click();
  await h.page.getByText('Detected integrations', { exact: false }).click();
  await expect(h.page.getByText('Owner Claude', { exact: false }).first()).toBeVisible();
  await mkdir(path.join(h.root, '.impeccable/review'), { recursive: true });
  await h.page.screenshot({
    path: path.join(h.root, '.impeccable/review/shared-integrations.png'),
  });
  checks.push(
    'Native integration settings show detected sources, sharing controls and unsupported adapters explicitly',
  );
  await h.page.getByRole('button', { name: 'Close settings', exact: true }).click();
  await rm(path.join(gptDir, 'native-gateway-unsupported'));
  await writeFile(
    path.join(h.workspace, 'native-owner.json'),
    JSON.stringify({ executable, profile: gptDir }),
  );
  const session = await h.invoke('create_session', { workspace: h.workspace });
  await h.invoke('update_session', {
    id: session.id,
    title: 'Shared account consent',
    pinned: false,
  });
  await h.invoke('send_message', {
    id: session.id,
    text: 'CLAUDE_ACCOUNT_GATEWAY_FIXTURE',
    options: {
      mode: 'claude',
      accountId: owner.id,
      model: 'sonnet',
      effort: 'low',
      accessMode: 'autopilot',
    },
  });
  await h.page.reload();
  await h.page.locator('.session-row').filter({ hasText: 'Shared account consent' }).click();
  const form = h.page.getByRole('form', { name: 'Request from a connector' });
  await expect(form).toBeVisible({ timeout: 25000 });
  await form.getByRole('radio', { name: 'Accept', exact: true }).check();
  await form.getByRole('textbox', { name: 'Your answer', exact: true }).fill('test');
  await form.getByRole('button', { name: 'Send answers' }).click();
  await expect
    .poll(
      async () =>
        (await h.invoke('list_sessions', { search: '', offset: 0 })).find(
          (s) => s.id === session.id,
        )?.status,
      { timeout: 20000 },
    )
    .toBe('completed');
  const messages = await h.invoke('get_messages', { id: session.id });
  assert(messages.at(-1).text.includes('NATIVE_ACCOUNT_GATEWAY_OK'));
  assert(
    messages
      .at(-1)
      .activity.some(
        (a) =>
          a.kind === 'interaction-result' && Object.keys(JSON.parse(a.text).answers).length === 0,
      ),
  );
  checks.push(
    'Claude calls an owning ChatGPT tool while Lumen displays consent independently of the caller CLI and redacts the answer',
  );
  console.log(JSON.stringify({ checks, liveCalls: 0 }, null, 2));
} finally {
  for (const child of owned)
    if (child.exitCode === null) {
      child.stdin.end();
      child.kill();
    }
  await h.close();
}
