// Official ACP/policy API fixtures. All state is owned by an isolated Lumen test root.
import { existsSync, readFileSync, writeFileSync, mkdirSync, appendFileSync } from 'node:fs';
import path from 'node:path';
import { createInterface } from 'node:readline';
import { spawn } from 'node:child_process';
const root = path.join(process.env.LUMEN_TEST_DATA_DIR, 'native-opencode-fixture');
mkdirSync(root, { recursive: true });
const argv = process.argv.slice(2);
const statePath = (id) => path.join(root, id + '.json');
const load = (id) => JSON.parse(readFileSync(statePath(id), 'utf8'));
const save = (s) => writeFileSync(statePath(s.id), JSON.stringify(s));
const out = (v) => process.stdout.write(JSON.stringify(v) + '\n');
if (argv[0] === 'api') {
  const method = argv.find((x) => ['GET', 'POST', 'PATCH'].includes(x));
  const route = argv[argv.indexOf(method) + 1];
  const data = argv.includes('--data') ? JSON.parse(argv[argv.indexOf('--data') + 1]) : {};
  // Catalogue and Go quota for the Auto emergency chain.
  if (method === 'GET' && route === '/api/provider') {
    out({
      data: [
        { id: 'opencode-go', name: 'OpenCode Go', activation: 'enabled' },
        { id: 'opencode', name: 'Default / OpenCode', activation: 'enabled' },
        { id: 'openrouter', name: 'OpenRouter', activation: 'auto' },
      ],
    });
    process.exit(0);
  }
  if (method === 'GET' && route === '/api/model/default') {
    out({ data: { id: 'fixture-free', providerID: 'opencode' } });
    process.exit(0);
  }
  if (method === 'GET' && route === '/api/model') {
    const model = (providerID, id, name, cost, variants = []) => ({
      id,
      providerID,
      type: 'chat',
      name,
      enabled: true,
      capabilities: { tools: true },
      variants: variants.map((v) => ({ id: v })),
      cost: [{ input: cost, output: cost }],
    });
    out({
      data: [
        model('opencode-go', 'claude-haiku-5-5', 'Claude Haiku 5.5', 0.1, ['none', 'low', 'medium', 'high', 'xhigh', 'max']),
        model('opencode-go', 'gpt-6-luna', 'GPT-6 Luna', 0.1, ['none', 'low', 'medium', 'high']),
        model('opencode-go', 'deepseek-v4.1-flash', 'DeepSeek V4.1 Flash 1017', 0.15, ['low', 'high', 'max']),
        model('opencode-go', 'minimax-m3.1-flash', 'MiniMax M3.1 Flash', 0.15, ['low', 'medium', 'high']),
        model('opencode-go', 'qwen4-max', 'Qwen4 Max', 1.5, ['none', 'low', 'high', 'xhigh']),
        model('opencode-go', 'grok-4.7', 'Grok 4.7', 2, ['low', 'medium', 'high', 'xhigh']),
        model('opencode-go', 'kimi-k3.1-pro', 'Kimi K3.1 Pro', 2, ['low', 'high', 'max']),
        model('opencode', 'fixture-free', 'Fixture Free', 0),
        model('opencode', 'gpt-6.2-sol', 'GPT-6.2 Sol', 2),
        model('openrouter', 'solar-mini', 'Solar Mini', 0.1),
      ],
    });
    process.exit(0);
  }
  if (method === 'GET' && route === '/api/experimental/usage/limits') {
    const limited = existsSync(path.join(process.env.LUMEN_TEST_DATA_DIR, 'opencode-go-limited'));
    const at = Date.now();
    const meter = (minutes) => ({ percent: 10, limited, resetsAt: at + minutes * 60_000 });
    out({
      data: {
        workspace: { id: 'wrk_fixture', name: 'Fixture' },
        go: { rolling: meter(300), weekly: meter(10_080), monthly: meter(43_200) },
        budgets: [],
      },
    });
    process.exit(0);
  }
  if (method === 'POST') {
    const s = {
      id: 'ses_' + crypto.randomUUID().replaceAll('-', ''),
      location: data.location,
      permissions: data.permissions,
      turns: 0,
    };
    save(s);
    out({ data: s });
  } else {
    const id = route.split('/').at(-1);
    try {
      const s = load(id);
      if (method === 'PATCH') {
        s.permissions = data.permissions;
        save(s);
      }
      out({ data: s });
    } catch {
      out({ error: { message: 'Session not found' } });
    }
  }
  process.exit(0);
}
let state,
  servers = [];
const permissions = new Map();
const reply = (id, result) => out({ jsonrpc: '2.0', id, result });
const update = (value) =>
  out({ jsonrpc: '2.0', method: 'session/update', params: { sessionId: state.id, update: value } });
const text = (t) =>
  update({ sessionUpdate: 'agent_message_chunk', content: { type: 'text', text: t } });
function effect(action) {
  let effect = 'allow';
  for (const r of state.permissions ?? []) {
    if (r.action === '*' || r.action === action) effect = r.effect;
  }
  return effect;
}
async function mcp(server, name, args) {
  const child = spawn(server.command, server.args, {
    cwd: process.cwd(),
    windowsHide: true,
    env: { ...process.env, ...Object.fromEntries(server.env.map((x) => [x.name, x.value])) },
    stdio: 'pipe',
  });
  let id = 0;
  const pending = new Map();
  const lines = createInterface({ input: child.stdout });
  lines.on('line', (line) => {
    const v = JSON.parse(line);
    if (v.id !== undefined) {
      pending.get(v.id)?.(v);
      pending.delete(v.id);
    }
  });
  const rpc = (method, params = {}) =>
    new Promise((resolve) => {
      const current = ++id;
      pending.set(current, resolve);
      child.stdin.write(JSON.stringify({ jsonrpc: '2.0', id: current, method, params }) + '\n');
    });
  try {
    await rpc('initialize', {
      protocolVersion: '2025-06-18',
      clientInfo: { name: 'fixture', version: '1' },
      capabilities: {},
    });
    child.stdin.write(
      JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' }) + '\n',
    );
    await rpc('tools/list');
    return (await rpc('tools/call', { name, arguments: args })).result;
  } finally {
    child.stdin.end();
    lines.close();
  }
}
async function prompt(v) {
  const current = v.id;
  out({
    jsonrpc: '2.0',
    method: 'session/update',
    params: {
      sessionId: 'ses_wrong',
      update: {
        sessionUpdate: 'agent_message_chunk',
        content: { type: 'text', text: 'WRONG_SESSION_OUTPUT' },
      },
    },
  });
  out({
    jsonrpc: '2.0',
    id: 9002,
    method: 'session/request_permission',
    params: {
      sessionId: 'ses_wrong',
      toolCall: { kind: 'edit', title: 'Wrong session action' },
      options: [{ optionId: 'once', kind: 'allow_once', name: 'Allow once' }],
    },
  });

  const prompt = v.params.prompt.map((p) => p.text || '').join('');
  const resumed = state.turns > 0;
  if (!resumed) state.mode = prompt;
  state.turns++;
  save(state);
  const mode = state.mode;
  if (mode.includes('OPENCODE_SLOW_FIXTURE')) {
    const child = spawn(process.execPath, ['-e', 'setTimeout(()=>{},60000)'], {
      windowsHide: true,
      stdio: 'ignore',
    });
    writeFileSync('opencode-pids.json', JSON.stringify([process.pid, child.pid]));
    text('Waiting');
    return;
  }
  if (mode.includes('OPENCODE_BROWSER_URL=')) {
    const url = mode.match(/OPENCODE_BROWSER_URL=(https?:\/\/[^\s]+)/)[1];
    const r = await mcp(
      servers.find((s) => s.name === 'lumen_browser'),
      'web_fetch',
      { url },
    );
    text(JSON.stringify(r));
    reply(current, { stopReason: 'end_turn' });
    return;
  }
  if (mode.includes('OPENCODE_GRAPH')) {
    const r = await mcp(
      servers.find((s) => s.name === 'lumen_graph'),
      'file_outline',
      { path: 'fixture.ts' },
    );
    text(JSON.stringify(r));
    reply(current, { stopReason: 'end_turn' });
    return;
  }
  if (mode.includes('OPENCODE_WRITE')) {
    const run = (allowed) => {
      if (allowed) {
        appendFileSync('opencode-owned-write.txt', 'once\n');
      }
      text(allowed ? 'WRITE_OK' : 'WRITE_DENIED');
      reply(current, { stopReason: 'end_turn' });
    };
    const rule = effect('edit');
    update({
      sessionUpdate: 'tool_call',
      toolCallId: 'write',
      title: 'Write project file',
      kind: 'edit',
      status: 'pending',
    });
    if (rule === 'ask') {
      permissions.set(9001, run);
      out({
        jsonrpc: '2.0',
        id: 9001,
        method: 'session/request_permission',
        params: {
          sessionId: state.id,
          toolCall: {
            toolCallId: 'write',
            kind: 'edit',
            title: 'Write project file',
            rawInput: { path: 'opencode-owned-write.txt' },
          },
          options: [
            { optionId: 'once', kind: 'allow_once', name: 'Allow once' },
            { optionId: 'deny', kind: 'reject_once', name: 'Deny' },
          ],
        },
      });
    } else run(rule === 'allow');
    return;
  }
  if (effect('edit') === 'allow' && !(mode.includes('OPENCODE_QUOTA') && resumed))
    appendFileSync('opencode-effects.txt', String(resumed) + '\n');
  update({
    sessionUpdate: 'tool_call',
    toolCallId: 'native-fixture-write',
    title: 'write',
    kind: 'edit',
    status: 'completed',
  });
  if (mode.includes('OPENCODE_QUOTA') && !resumed) {
    text('Partial work retained');
    out({
      jsonrpc: '2.0',
      id: current,
      error: { code: -32000, message: 'Quota exhausted. Try again in 1 second.' },
    });
    return;
  }
  text(resumed ? 'OPENCODE_RESUME_OK' : 'OPENCODE_FIRST_OK');
  reply(current, { stopReason: 'end_turn' });
}
for await (const line of createInterface({ input: process.stdin })) {
  const v = JSON.parse(line);
  if (v.method === 'initialize')
    reply(v.id, {
      protocolVersion: 1,
      agentCapabilities: { loadSession: true, mcpCapabilities: { http: true } },
    });
  else if (v.method === 'session/load') {
    state = load(v.params.sessionId);
    servers = v.params.mcpServers;
    if (
      servers.some(
        (s) => s.name !== 'lumen_browser' && s.env?.some((e) => e.name === 'LUMEN_BROWSER_CAP'),
      )
    )
      throw Error('Browser capability leaked to another MCP server');
    update({
      sessionUpdate: 'agent_message_chunk',
      content: { type: 'text', text: 'OLD_HISTORY_SHOULD_NOT_REPLAY' },
    });
    out({
      jsonrpc: '2.0',
      id: 9003,
      method: 'session/request_permission',
      params: {
        sessionId: state.id,
        toolCall: { kind: 'edit', title: 'Action before prompt' },
        options: [{ optionId: 'once', kind: 'allow_once', name: 'Allow once' }],
      },
    });
    reply(v.id, { configOptions: [] });
  } else if (v.method === 'session/set_config_option')
    reply(v.id, { configOptions: [{ id: v.params.configId, currentValue: v.params.value }] });
  else if (v.method === 'session/prompt') await prompt(v);
  else if (v.method === 'session/cancel') {
    permissions.clear();
  } else if ([9002, 9003].includes(v.id)) {
    if (v.result?.outcome?.outcome !== 'cancelled')
      throw Error('Unowned or premature permission was granted');
  } else if (v.id === 9001) {
    permissions.get(v.id)?.(
      v.result?.outcome?.outcome === 'selected' && v.result.outcome.optionId === 'once',
    );
    permissions.delete(v.id);
  }
}
