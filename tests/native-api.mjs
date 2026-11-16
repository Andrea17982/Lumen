// Actual WebView2/IPC/keychain/SQLite against three local official-wire fixtures. No paid inference.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { nativeHarness } from './native-harness.mjs';
const secret = 'lumen-fixture-not-a-real-key';
const generations = [];
const oracle = 'lumen-api-oracle-' + crypto.randomUUID();
const checks = [];
let fixtureMode = 'write';
const server = createServer(async (req, res) => {
  try {
    if (req.url === '/page') {
      res.setHeader('Content-Type', 'text/html');
      res.end(`<html><body>${oracle}</body></html>`);
      return;
    }
    const provider = req.url.split('/')[1];
    const auth =
      provider === 'openaiapi'
        ? req.headers.authorization
        : provider === 'anthropicapi'
          ? req.headers['x-api-key']
          : req.headers['x-goog-api-key'];
    assert.equal(auth, provider === 'openaiapi' ? `Bearer ${secret}` : secret);
    const reply = (v) => {
      res.setHeader('Content-Type', 'application/json');
      res.end(JSON.stringify(v));
    };
    if (req.method === 'GET') {
      reply(
        provider === 'googleapi'
          ? {
              models: [
                { name: 'models/fixture-model', supportedGenerationMethods: ['generateContent'] },
              ],
            }
          : { data: [{ id: 'fixture-model' }] },
      );
      return;
    }
    let raw = '';
    for await (const b of req) raw += b;
    const body = JSON.parse(raw);
    generations.push({ provider, mode: fixtureMode, body });
    if (fixtureMode === 'quota') {
      res.statusCode = 429;
      reply({ error: { message: 'fixture quota' } });
      return;
    }
    if (fixtureMode === 'slow') {
      setTimeout(() => {
        if (!res.destroyed) reply({});
      }, 15000).unref();
      return;
    }
    const history =
      provider === 'openaiapi'
        ? body.input
        : provider === 'anthropicapi'
          ? body.messages
          : body.contents;
    const hasResult = JSON.stringify(history).includes(
      fixtureMode === 'question'
        ? 'tool_result_never'
        : provider === 'openaiapi'
          ? 'function_call_output'
          : provider === 'anthropicapi'
            ? 'tool_result'
            : 'functionResponse',
    );
    if (provider === 'googleapi' && hasResult) {
      const called = history.flatMap((x) => x.parts || []).find((x) => x.functionCall)
        ?.functionCall.name;
      const returned = history.flatMap((x) => x.parts || []).find((x) => x.functionResponse)
        ?.functionResponse.name;
      assert.equal(returned, called, 'Function response must preserve the advertised alias');
    }
    const definitions =
      provider === 'openaiapi' || provider === 'anthropicapi'
        ? body.tools
        : body.tools[0].functionDeclarations;
    const shared = definitions.find((x) => x.description?.endsWith('Lumen API shared fixture'));
    const name =
      fixtureMode === 'question'
        ? 'lumen_ask_user'
        : fixtureMode === 'browser'
          ? 'web_fetch'
          : fixtureMode === 'graph'
            ? 'lumen_graph_file_outline'
            : fixtureMode === 'shared'
              ? shared?.name
              : ['shell', 'shell-slow', 'shell-error'].includes(fixtureMode)
                ? 'lumen_run_command'
                : 'lumen_write_file';
    const args =
      fixtureMode === 'question'
        ? { question: 'Pause API_AUTO_WAIT' }
        : fixtureMode === 'browser'
          ? { url: `http://127.0.0.1:${server.address().port}/page` }
          : fixtureMode === 'graph'
            ? { path: 'fixture.ts' }
            : fixtureMode === 'shared'
              ? { count: 0, enabled: false }
              : fixtureMode === 'shell'
                ? { command: 'echo shell-oracle > api-shell.txt' }
                : fixtureMode === 'shell-error'
                  ? { command: 'exit 7' }
                  : fixtureMode === 'shell-slow'
                    ? { command: 'node api-shell-slow.mjs' }
                    : { path: `api-${provider}.txt`, content: 'written once' };
    assert(name, 'Missing advertised shared tool');
    const text = JSON.stringify(history).includes('isError') ? 'DENIED' : 'API_OK';
    if (provider === 'openaiapi')
      reply({
        status: 'completed',
        output: hasResult
          ? [{ type: 'message', content: [{ type: 'output_text', text }] }]
          : [
              {
                type: 'function_call',
                call_id: 'fixture-call',
                name,
                arguments: JSON.stringify(args),
              },
            ],
        usage: { input_tokens: 10, output_tokens: 5 },
      });
    else if (provider === 'anthropicapi')
      reply({
        content: hasResult
          ? [{ type: 'text', text }]
          : [{ type: 'tool_use', id: 'fixture-call', name, input: args }],
        stop_reason: hasResult ? 'end_turn' : 'tool_use',
        usage: { input_tokens: 10, output_tokens: 5 },
      });
    else
      reply({
        candidates: [
          {
            content: {
              role: 'model',
              parts: hasResult
                ? [{ text }]
                : [
                    {
                      functionCall: { name, args },
                      thoughtSignature: 'preserve-fixture-signature',
                    },
                  ],
            },
            finishReason: 'STOP',
          },
        ],
        usageMetadata: { promptTokenCount: 10, candidatesTokenCount: 5 },
      });
  } catch (error) {
    res.statusCode = 500;
    res.end(JSON.stringify({ error: 'fixture assertion failed' }));
    console.error(error);
  }
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const discovery = await mkdtemp(path.join(tmpdir(), 'lumen-api-discovery-'));
const h = await nativeHarness('09-api', {
  appEnv: {
    LUMEN_TEST_API_BASE: `http://127.0.0.1:${server.address().port}/`,
    LUMEN_DISCOVERY_HOME: discovery,
    LUMEN_DISCOVERY_CLAUDE_HOME: discovery,
    LUMEN_DISCOVERY_CODEX_HOME: discovery,
  },
});
const accounts = [];
const sessions = [];
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const policy = {
  model: 'fixture-model',
  autoEnabled: false,
  budgetUsd: 1,
  inputPerMillion: 1,
  outputPerMillion: 2,
  maxOutputTokens: 256,
  ratesConfirmed: true,
};
const opts = (a, access = 'autopilot') => ({
  mode: a.provider,
  accountId: a.id,
  model: 'fixture-model',
  effort: '',
  accessMode: access,
  autoReview: false,
});
async function chat() {
  const s = await h.invoke('create_session', { workspace: h.workspace });
  sessions.push(s.id);
  return s;
}
async function done(id) {
  for (let i = 0; i < 400; i++) {
    const s = (await h.invoke('list_sessions', { search: '', offset: 0 })).find((x) => x.id === id);
    if (['completed', 'error', 'interrupted'].includes(s.status))
      return { s, m: (await h.invoke('get_messages', { id })).at(-1) };
    await sleep(50);
  }
  throw Error('API turn did not finish');
}
async function pending(id) {
  for (let i = 0; i < 200; i++) {
    const m = (await h.invoke('get_messages', { id })).at(-1);
    const row = m?.activity.find((x) => x.kind === 'interaction');
    if (row) return JSON.parse(row.text);
    await sleep(50);
  }
  throw Error('No host question');
}
try {
  await h.invoke('save_settings', {
    value: {
      ...(await h.invoke('get_settings')),
      verifyChanges: false,
      repoContext: false,
      orchestra: false,
      closeToTray: false,
    },
  });
  await writeFile(
    path.join(h.workspace, 'fixture.ts'),
    `export function ${oracle.replaceAll('-', '_')}(){return 1;}`,
  );
  const mcp = path.join(h.workspace, 'fixture-mcp.mjs');
  await writeFile(
    mcp,
    `import{createInterface}from'node:readline';for await(const line of createInterface({input:process.stdin})){const v=JSON.parse(line);const result=v.method==='initialize'?{protocolVersion:'2025-06-18',serverInfo:{name:'fixture',version:'1'},capabilities:{tools:{}}}:v.method==='tools/list'?{tools:[{name:'echo',description:'Lumen API shared fixture',inputSchema:{type:'object'},annotations:{readOnlyHint:true,idempotentHint:true}}]}:v.method==='tools/call'?{content:[{type:'text',text:JSON.stringify({oracle:${JSON.stringify(oracle)},args:v.params.arguments})}]}:{};if(v.id!==undefined)console.log(JSON.stringify({jsonrpc:'2.0',id:v.id,result}));}`,
  );
  await writeFile(
    path.join(h.workspace, '.mcp.json'),
    JSON.stringify({ mcpServers: { fixture: { command: process.execPath, args: [mcp] } } }),
  );
  for (const provider of ['openaiapi', 'anthropicapi', 'googleapi']) {
    const a = await h.invoke('add_account', { provider, name: `${provider} fixture` });
    accounts.push(a);
    await h.invoke('save_api_key', { id: a.id, secret });
    await h.invoke('save_api_settings', { id: a.id, value: policy });
    assert.equal((await h.invoke('list_models', { id: a.id }))[0].id, 'fixture-model');
    const state = await h.invoke('get_api_settings', { id: a.id });
    assert(state.keySaved);
    assert(!JSON.stringify(state).includes(secret));
    const s = await chat();
    await h.invoke('send_message', { id: s.id, text: 'API_WRITE_FIXTURE', options: opts(a) });
    const r = await done(s.id);
    assert.equal(r.s.status, 'completed');
    assert.equal(r.m.text, 'API_OK');
    assert(r.m.activity.some((x) => x.kind === 'api-cost'));
    assert.equal(
      await readFile(path.join(h.workspace, `api-${provider}.txt`), 'utf8'),
      'written once',
    );
    const read = await chat();
    await h.invoke('send_message', {
      id: read.id,
      text: 'API_READONLY_FORCE_WRITE',
      options: opts(a, 'read-only'),
    });
    const denied = await done(read.id);
    assert.equal(denied.m.text, 'DENIED');
    const ask = await chat();
    await h.invoke('send_message', {
      id: ask.id,
      text: 'API_ASK_FORCE_WRITE',
      options: opts(a, 'ask'),
    });
    const q = await pending(ask.id);
    assert.equal(q.kind, 'permission');
    await h.invoke('answer_question', { id: q.id, answers: { action: ['Deny'] } });
    assert.equal((await done(ask.id)).m.text, 'DENIED');
    const edits = await chat();
    await h.invoke('send_message', {
      id: edits.id,
      text: 'API_FILE_EDITS',
      options: opts(a, 'file-edits'),
    });
    assert.equal((await done(edits.id)).m.text, 'API_OK');
    const allowed = await chat();
    await h.invoke('send_message', {
      id: allowed.id,
      text: 'API_ASK_ALLOW_ONCE',
      options: opts(a, 'ask'),
    });
    const once = await pending(allowed.id);
    assert.notEqual(once.id, q.id);
    await h.invoke('answer_question', { id: once.id, answers: { action: ['Allow once'] } });
    assert.equal((await done(allowed.id)).m.text, 'API_OK');
    for (const mode of ['browser', 'graph', 'shared']) {
      fixtureMode = mode;
      const probe = await chat();
      await h.invoke('send_message', {
        id: probe.id,
        text: 'API_TOOL_FIXTURE',
        options: opts(a, 'read-only'),
      });
      assert.equal((await done(probe.id)).s.status, 'completed');
      assert(
        generations.some(
          (g) =>
            g.provider === provider &&
            g.mode === mode &&
            JSON.stringify(g.body).includes(
              mode === 'graph' ? oracle.replaceAll('-', '_') : oracle,
            ),
        ),
      );
    }
    fixtureMode = 'write';
    checks.push(
      `${provider}: auth/catalog, real keychain, all four access profiles, one-shot approval, Moli/graph/shared-MCP tool-result oracles and cost activity`,
    );
  }
  assert(
    generations.some(
      (x) =>
        x.provider === 'googleapi' && JSON.stringify(x.body).includes('preserve-fixture-signature'),
    ),
  );
  const a = accounts[0];
  fixtureMode = 'shell';
  for (const profile of ['read-only', 'file-edits']) {
    const s = await chat();
    await h.invoke('send_message', {
      id: s.id,
      text: 'API forced shell',
      options: opts(a, profile),
    });
    assert.equal((await done(s.id)).m.text, 'DENIED');
    await assert.rejects(readFile(path.join(h.workspace, 'api-shell.txt')));
  }
  const allowed = await chat();
  await h.invoke('send_message', { id: allowed.id, text: 'API shell', options: opts(a, 'ask') });
  const approval = await pending(allowed.id);
  await h.invoke('answer_question', { id: approval.id, answers: { action: ['Allow once'] } });
  assert.equal((await done(allowed.id)).m.text, 'API_OK');
  assert(
    (await readFile(path.join(h.workspace, 'api-shell.txt'), 'utf8')).includes('shell-oracle'),
  );
  await writeFile(
    path.join(h.workspace, 'api-shell-slow.mjs'),
    `import{spawn}from'node:child_process';import{writeFileSync}from'node:fs';const child=spawn(process.execPath,['-e','setTimeout(()=>{},60000)'],{windowsHide:true,stdio:'ignore'});writeFileSync('api-shell-pids.json',JSON.stringify([process.pid,child.pid]));setTimeout(()=>{},60000);`,
  );
  fixtureMode = 'shell-error';
  const commandError = await chat();
  await h.invoke('send_message', {
    id: commandError.id,
    text: 'API failed command',
    options: opts(a),
  });
  assert.equal((await done(commandError.id)).m.text, 'DENIED');
  fixtureMode = 'shell-slow';
  const running = await chat();
  await h.invoke('send_message', {
    id: running.id,
    text: 'API cancellable shell',
    options: opts(a),
  });
  let pids;
  for (let i = 0; i < 150; i++) {
    try {
      pids = JSON.parse(await readFile(path.join(h.workspace, 'api-shell-pids.json'), 'utf8'));
      break;
    } catch {}
    await sleep(50);
  }
  assert.equal(pids.length, 2);
  await h.invoke('cancel_run', { id: running.id });
  assert.equal((await done(running.id)).s.status, 'interrupted');
  for (let i = 0; i < 100; i++) {
    if (
      pids.every((pid) => {
        try {
          process.kill(pid, 0);
          return false;
        } catch {
          return true;
        }
      })
    )
      break;
    await sleep(50);
  }
  for (const pid of pids) assert.throws(() => process.kill(pid, 0));
  checks.push(
    'Host shell enforces restricted profiles and one-shot Ask; Stop kills both owned command descendants',
  );
  fixtureMode = 'write';
  const blocked = await chat();
  const before = generations.length;
  await assert.rejects(
    h.invoke('send_message', {
      id: blocked.id,
      text: 'Hi',
      options: { mode: 'auto', effort: '', accessMode: 'read-only', autoReview: false },
    }),
  );
  assert.equal(generations.length, before);
  await h.invoke('save_api_settings', { id: a.id, value: { ...policy, autoEnabled: true } });
  fixtureMode = 'question';
  const paused = await chat();
  await h.invoke('send_message', {
    id: paused.id,
    text: 'Hi API_AUTO_WAIT',
    options: { mode: 'auto', effort: '', accessMode: 'autopilot', autoReview: false },
  });
  const question = await pending(paused.id);
  const sent = generations.length;
  await h.invoke('save_api_settings', { id: a.id, value: policy });
  await h.invoke('answer_question', { id: question.id, answers: { answer: ['Continue'] } });
  assert.equal((await done(paused.id)).s.status, 'error');
  assert.equal(generations.length, sent);
  checks.push('Auto requires opt-in; revocation during a tool turn prevents the next paid request');
  fixtureMode = 'write';
  await h.invoke('save_api_settings', { id: a.id, value: { ...policy, budgetUsd: 0.000001 } });
  const exhausted = await chat();
  const count = generations.length;
  await h.invoke('send_message', { id: exhausted.id, text: 'API_BUDGET_BLOCK', options: opts(a) });
  assert.equal((await done(exhausted.id)).s.status, 'error');
  assert.equal(generations.length, count);
  await h.invoke('save_api_settings', { id: a.id, value: policy });
  checks.push('Budget exhaustion blocks inference before HTTP and does not reset prior charges');
  fixtureMode = 'quota';
  const limited = await chat();
  await h.invoke('send_message', { id: limited.id, text: 'API_QUOTA', options: opts(a) });
  const quota = await done(limited.id);
  assert.equal(quota.s.status, 'error');
  assert(
    quota.m.activity.some((x) => x.kind === 'api-cost' && JSON.parse(x.text).status === 'rejected'),
  );
  fixtureMode = 'slow';
  const stopped = await chat();
  await h.invoke('send_message', { id: stopped.id, text: 'API_CANCEL', options: opts(a) });
  for (let i = 0; i < 100 && !generations.some((x) => x.mode === 'slow'); i++) await sleep(30);
  await h.invoke('cancel_run', { id: stopped.id });
  assert.equal((await done(stopped.id)).s.status, 'interrupted');
  const budget = await h.invoke('get_api_settings', { id: a.id });
  assert(budget.spentUsd > 0);
  assert(!(await readFile(path.join(h.data, 'api.sqlite'))).includes(Buffer.from(secret)));
  checks.push(
    'Known HTTP rejections release reservations; Stop retains uncertain charges; no API key in SQLite',
  );
  await h.page.reload();
  await h.page.getByRole('button', { name: 'Settings', exact: true }).click();
  await h.page.getByRole('tab', { name: 'Accounts', exact: true }).click();
  const panel = h.page.locator('.api-account-settings').first();
  await panel.locator('summary').click();
  const keyInput = panel.locator('input[type=password]');
  assert.equal(await keyInput.inputValue(), '');
  await keyInput.fill(secret);
  await panel.getByRole('button', { name: 'Save key', exact: true }).click();
  await h.page.waitForFunction(
    () => document.querySelector('.api-account-settings input[type=password]')?.value === '',
  );
  assert.equal(await panel.getByLabel('Priced model ID').inputValue(), 'fixture-model');
  await panel.getByLabel('Priced model ID').fill('fixture-model-changed');
  assert.equal(
    await panel.getByRole('checkbox', { name: /I checked the highest/ }).isChecked(),
    false,
  );
  await panel.getByLabel('Priced model ID').fill('fixture-model');
  await panel.getByRole('checkbox', { name: /I checked the highest/ }).check();
  await panel.getByRole('button', { name: 'Save API settings', exact: true }).click();
  await panel.getByRole('status').filter({ hasText: 'API settings saved.' }).waitFor();
  assert.equal((await h.invoke('get_api_settings', { id: a.id })).config.model, 'fixture-model');

  for (const width of [480, 1276])
    for (const theme of ['light', 'dark']) {
      await h.page.setViewportSize({ width, height: 1000 });
      await h.page.evaluate((theme) => (document.documentElement.dataset.theme = theme), theme);
      assert(
        await panel.evaluate((e) => e.scrollWidth <= e.clientWidth + 1),
        'API settings overflow',
      );
      await h.page.screenshot({
        path: `.codex/verification/09-api-settings-${width}-${theme}.png`,
      });
    }
  checks.push(
    'Native account settings render in both themes at 480/1276 px; saving a key clears the password field',
  );
  await writeFile(
    '.codex/verification/09-api-native.json',
    JSON.stringify(
      {
        passed: true,
        checks,
        generationRequests: generations.length,
        scope:
          'Local HTTP fixtures; actual Windows WebView2/IPC/keychain/SQLite. No real key, login or paid inference.',
      },
      null,
      2,
    ),
  );
  console.log(checks.join('\n'));
} finally {
  for (const id of sessions) await h.invoke('cancel_run', { id }).catch(() => {});
  await sleep(200);
  for (const a of accounts)
    for (let i = 0; i < 30; i++) {
      try {
        await h.invoke('remove_account', { id: a.id });
        break;
      } catch {
        await sleep(100);
      }
    }
  await h.close();
  server.closeAllConnections();
  await new Promise((r) => server.close(r));
}
