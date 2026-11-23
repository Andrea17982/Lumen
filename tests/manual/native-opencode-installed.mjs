// Installed OpenCode ACP + deterministic local model; no paid provider request is permitted.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { readFile, writeFile, mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { nativeHarness } from './native-harness.mjs';
let action, target, mode;
const requests = [],
  checks = [];
const fixtureErrors = [];
const server = createServer(async (req, res) => {
  try {
    let raw = '';
    for await (const b of req) raw += b;
    const body = JSON.parse(raw);
    assert.equal(body.model, 'fixture');
    assert.equal(req.headers.authorization, 'Bearer lumen-local-fixture');
    requests.push({
      mode,
      toolNames: (body.tools || []).map((x) => x.function.name),
      messages: body.messages,
    });
    const hasResult = body.messages.some((x) => x.role === 'tool');
    const toolName =
      action === 'mcp'
        ? body.tools.find((t) => t.function.name?.endsWith('_oracle'))?.function.name
        : action;
    if (!hasResult) assert(toolName);
    const delta = hasResult
      ? { content: 'NATIVE_LOCAL_DONE' }
      : {
          role: 'assistant',
          tool_calls: [
            {
              index: 0,
              id: 'call_local',
              type: 'function',
              function: {
                name: toolName,
                arguments: JSON.stringify(
                  action === 'mcp'
                    ? {}
                    : action === 'shell'
                      ? { command: 'echo unexpected > forbidden-shell.txt' }
                      : { path: target, content: 'native-write-oracle' },
                ),
              },
            },
          ],
        };
    res.setHeader('Content-Type', 'text/event-stream');
    const emit = (x) =>
      res.write(
        'data: ' +
          JSON.stringify({
            id: 'chatcmpl-local',
            object: 'chat.completion.chunk',
            created: 1,
            model: 'fixture',
            ...x,
          }) +
          '\n\n',
      );
    emit({ choices: [{ index: 0, delta, finish_reason: null }] });
    emit({
      choices: [{ index: 0, delta: {}, finish_reason: hasResult ? 'stop' : 'tool_calls' }],
      usage: { prompt_tokens: 10, completion_tokens: 5, total_tokens: 15 },
    });
    res.end('data: [DONE]\n\n');
  } catch (e) {
    fixtureErrors.push(String(e));
    console.error(String(e));
    res.statusCode = 400;
    res.end('Local fixture failed');
  }
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const discovery = await mkdtemp(path.join(tmpdir(), 'lumen-oc-discovery-'));
const h = await nativeHarness('09-opencode-installed', {
  offline: false,
  appEnv: {
    LUMEN_DISCOVERY_HOME: discovery,
    LUMEN_DISCOVERY_CLAUDE_HOME: discovery,
    LUMEN_DISCOVERY_CODEX_HOME: discovery,
  },
});
const outside = path.join(await mkdtemp(path.join(tmpdir(), 'lumen-oc-boundary-')), 'outside.txt');
const sessions = [];
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function done(id) {
  for (let i = 0; i < 900; i++) {
    const s = (await h.invoke('list_sessions', { search: '', offset: 0 })).find((x) => x.id === id);
    if (['completed', 'error', 'interrupted'].includes(s.status))
      return { s, m: (await h.invoke('get_messages', { id })).at(-1) };
    await sleep(50);
  }
  throw Error('Installed OpenCode timed out');
}
async function question(id) {
  for (let i = 0; i < 400; i++) {
    const row = (await h.invoke('get_messages', { id }))
      .at(-1)
      ?.activity.find((x) => x.kind === 'interaction');
    if (row) return JSON.parse(row.text);
    await sleep(50);
  }
  throw Error('Installed OpenCode did not ask');
}
try {
  await writeFile(
    path.join(h.workspace, 'opencode.json'),
    JSON.stringify({
      model: 'lumenfixture/fixture',
      update: 'disable',
      share: 'disabled',
      policies: [
        { action: 'provider.use', resource: '*', effect: 'deny' },
        { action: 'provider.use', resource: 'lumenfixture', effect: 'allow' },
      ],
      providers: {
        lumenfixture: {
          package: '@opencode/ai/providers/openai-compatible',
          env: [],
          settings: {
            baseURL: `http://127.0.0.1:${server.address().port}/v1`,
            apiKey: 'lumen-local-fixture',
          },
          models: {
            fixture: {
              name: 'Offline fixture',
              capabilities: { tools: true, input: ['text'], output: ['text'] },
              limit: { context: 32768, output: 4096 },
            },
          },
        },
      },
    }),
  );
  await h.invoke('save_settings', {
    value: {
      ...(await h.invoke('get_settings')),
      verifyChanges: false,
      repoContext: false,
      orchestra: false,
      closeToTray: false,
    },
  });
  const mcpFile = path.join(h.workspace, 'oc-mcp-fixture.mjs');
  const oracle = 'oc-mcp-' + crypto.randomUUID();
  await writeFile(
    mcpFile,
    `import{createInterface}from'node:readline';import{appendFileSync}from'node:fs';for await(const line of createInterface({input:process.stdin})){const v=JSON.parse(line);let r={};if(v.method==='initialize')r={protocolVersion:'2025-06-18',serverInfo:{name:'own',version:'1'},capabilities:{tools:{}}};if(v.method==='tools/list')r={tools:[{name:'oracle',description:'OC owned MCP oracle',inputSchema:{type:'object',properties:{}}}]};if(v.method==='tools/call'){appendFileSync(${JSON.stringify(path.join(h.workspace, 'mcp-effects.txt'))},'once\\n');r={content:[{type:'text',text:${JSON.stringify(oracle)}}]}}if(v.id!==undefined)console.log(JSON.stringify({jsonrpc:'2.0',id:v.id,result:r}));}`,
  );
  await writeFile(
    path.join(h.workspace, '.mcp.json'),
    JSON.stringify({ mcpServers: { own: { command: process.execPath, args: [mcpFile] } } }),
  );
  const a = await h.invoke('add_account', { provider: 'opencode', name: 'Own local fixture' });
  async function send(profile, tool, file) {
    console.log('Installed probe', profile, tool);
    mode = profile + '-' + tool + '-' + sessions.length;
    action = tool;
    target = file;
    const s = await h.invoke('create_session', { workspace: h.workspace });
    sessions.push(s.id);
    await h.invoke('send_message', {
      id: s.id,
      text: 'Offline permission fixture.',
      options: {
        mode: 'opencode',
        accountId: a.id,
        model: 'lumenfixture/fixture',
        effort: '',
        accessMode: profile,
        autoReview: false,
      },
    });
    return s.id;
  }
  let id = await send('read-only', 'write', 'readonly.txt');
  await done(id);
  await assert.rejects(readFile(path.join(h.workspace, 'readonly.txt')));
  assert(!requests.find((x) => x.mode.startsWith('read-only')).toolNames.includes('write'));
  checks.push('Installed OpenCode Read only omits write and denies a forced write');
  id = await send('file-edits', 'write', 'edits.txt');
  assert.equal((await done(id)).s.status, 'completed');
  assert.equal(await readFile(path.join(h.workspace, 'edits.txt'), 'utf8'), 'native-write-oracle');
  id = await send('file-edits', 'shell', '');
  await done(id);
  await assert.rejects(readFile(path.join(h.workspace, 'forbidden-shell.txt')));
  id = await send('file-edits', 'write', outside);
  await done(id);
  await assert.rejects(readFile(outside));
  checks.push(
    'Installed File edits writes internally, denies forced shell and denies an external-directory write',
  );
  id = await send('ask', 'write', 'ask-denied.txt');
  let q = await question(id);
  assert.equal(q.kind, 'permission');
  await h.invoke('answer_question', { id: q.id, answers: { action: ['Deny'] } });
  await done(id);
  await assert.rejects(readFile(path.join(h.workspace, 'ask-denied.txt')));
  id = await send('ask', 'write', 'ask-allowed.txt');
  q = await question(id);
  await h.invoke('answer_question', { id: q.id, answers: { action: ['Allow once'] } });
  assert.equal((await done(id)).s.status, 'completed');
  assert.equal(
    await readFile(path.join(h.workspace, 'ask-allowed.txt'), 'utf8'),
    'native-write-oracle',
  );
  checks.push(
    'Installed ACP asks before a write; Deny and Allow once reach the actual native tool',
  );
  id = await send('autopilot', 'write', 'autopilot.txt');
  assert.equal((await done(id)).s.status, 'completed');
  assert.equal(
    await readFile(path.join(h.workspace, 'autopilot.txt'), 'utf8'),
    'native-write-oracle',
  );
  checks.push('Installed Autopilot completes a local write through the selected project model');
  id = await send('ask', 'mcp', '');
  const mcpPermission = await question(id);
  await h.invoke('answer_question', { id: mcpPermission.id, answers: { action: ['Allow once'] } });
  const first = await done(id);
  assert.equal(first.s.status, 'completed');
  assert(
    requests
      .filter((x) => x.mode === mode)
      .some((x) => JSON.stringify(x.messages).includes(oracle)),
  );
  const effects = await readFile(path.join(h.workspace, 'mcp-effects.txt'), 'utf8');
  assert.equal(effects, 'once\n');
  await h.invoke('send_message', {
    id,
    text: 'Prosegui.',
    options: {
      mode: 'opencode',
      accountId: a.id,
      model: 'lumenfixture/fixture',
      effort: '',
      accessMode: 'ask',
      autoReview: false,
    },
  });
  const resumed = await done(id);
  assert.equal(resumed.s.status, 'completed');
  assert.equal(resumed.m.text, 'NATIVE_LOCAL_DONE');
  assert.equal(await readFile(path.join(h.workspace, 'mcp-effects.txt'), 'utf8'), effects);
  checks.push(
    'Installed ACP receives a shared project MCP result and resumes native history without repeating its effect',
  );
  assert.deepEqual(fixtureErrors, []);
  await writeFile(
    '.codex/verification/09-opencode-installed.json',
    JSON.stringify(
      {
        passed: true,
        checks,
        requests,
        scope:
          'Installed OpenCode 2.2.7, real ACP/session policy API, local model fixture; project policy denies all other providers. No paid inference.',
      },
      null,
      2,
    ),
  );
  console.log(checks.join('\n'));
} finally {
  for (const id of sessions) await h.invoke('cancel_run', { id }).catch(() => {});
  await h.close();
  server.closeAllConnections();
  await new Promise((r) => server.close(r));
}
