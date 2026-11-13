// Real Windows UI/IPC, fake local Ollama API and real owned MCP subprocesses. No inference/login.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { mkdir, writeFile, readFile } from 'node:fs/promises';
import path from 'node:path';
import { expect } from '@playwright/test';
import { nativeHarness } from './native-harness.mjs';

const failures = [];
let ownerId;
let httpClosed = 0;
const api = createServer(async (request, response) => {
  try {
    const chunks = [];
    for await (const chunk of request) chunks.push(chunk);
    const body = chunks.length ? JSON.parse(Buffer.concat(chunks)) : {};
    if (request.url === '/mcp') {
      assert.equal(request.headers.authorization, 'Bearer localhost-fixture');
      if (request.method === 'GET') {
        response.writeHead(405).end();
        return;
      }
      if (request.method === 'DELETE') {
        httpClosed++;
        response.end();
        return;
      }
      response.setHeader('Content-Type', 'application/json');
      if (body.method === 'initialize') {
        response.setHeader('mcp-session-id', 'original-http');
        response.end(
          JSON.stringify({
            jsonrpc: '2.0',
            id: body.id,
            result: {
              protocolVersion: body.params.protocolVersion,
              capabilities: { tools: {} },
              serverInfo: { name: 'http-fixture', version: '1' },
            },
          }),
        );
        return;
      }
      assert.equal(request.headers['mcp-session-id'], 'original-http');
      if (body.method === 'notifications/initialized') {
        response.writeHead(202).end();
        return;
      }
      const result =
        body.method === 'tools/list'
          ? {
              tools: [
                {
                  name: 'http_echo',
                  description: 'Original HTTP service',
                  inputSchema: { type: 'object' },
                  annotations: { readOnlyHint: true },
                },
              ],
            }
          : {
              content: [
                {
                  type: 'text',
                  text: JSON.stringify({
                    owner: 'original-http',
                    arguments: body.params.arguments,
                  }),
                },
              ],
            };
      response.end(JSON.stringify({ jsonrpc: '2.0', id: body.id, result }));
      return;
    }
    const native = body.model !== 'schema-fixture';
    if (request.url === '/api/tags') {
      response.end(
        JSON.stringify({
          models: ['local-fixture', 'schema-fixture'].map((name) => ({
            name,
            size: 1000,
            capabilities: ['completion', 'tools'],
          })),
        }),
      );
      return;
    }
    if (request.url === '/api/show') {
      response.end(
        JSON.stringify({ capabilities: native ? ['completion', 'tools'] : ['completion'] }),
      );
      return;
    }
    assert.equal(request.url, '/api/chat');
    const previous = body.messages.at(-1);
    const gateway = body.messages.some((message) => message.content?.includes('USE_OWNER_GATEWAY'));
    const http = body.messages.some((message) => message.content?.includes('USE_HTTP_SHARED'));
    let message;
    if (!['tool', 'user'].includes(previous.role) || !previous.tool_name) {
      const name = native
        ? body.tools.find((tool) =>
            tool.function.description.includes(
              http ? '/http_echo:' : gateway ? 'owner-service/read:' : '/echo:',
            ),
          ).function.name
        : body.format.properties.tool.enum.find((name) => name.startsWith('shared_'));
      const args = http
        ? { count: 13, enabled: false }
        : gateway
          ? { count: 11 }
          : native
            ? { elicit: true }
            : { count: 3, enabled: false };
      message = native
        ? { role: 'assistant', content: '', tool_calls: [{ function: { name, arguments: args } }] }
        : {
            role: 'assistant',
            content: JSON.stringify({ action: 'tool', tool: name, arguments: args, text: '' }),
          };
    } else {
      const result = JSON.parse(previous.content);
      assert(!result.isError, previous.content);
      if (http)
        assert.deepEqual(JSON.parse(result.content[0].text), {
          owner: 'original-http',
          arguments: { count: 13, enabled: false },
        });
      else if (gateway)
        assert.deepEqual(JSON.parse(result.content[0].text), {
          owner: ownerId,
          tool: 'read',
          arguments: { count: 11 },
        });
      else if (native) assert.equal(result.content[0].text, 'Consent accept');
      else
        assert.deepEqual(JSON.parse(result.content[0].text), {
          owner: 'claude-original',
          arguments: { count: 3, enabled: false },
        });
      message = {
        role: 'assistant',
        content: native
          ? 'LOCAL_SHARED_VERIFIED'
          : JSON.stringify({ action: 'final', text: 'LOCAL_SHARED_VERIFIED' }),
      };
    }
    response.setHeader('Content-Type', 'application/x-ndjson');
    response.end(JSON.stringify({ message, done: true }) + '\n');
  } catch (error) {
    failures.push(String(error));
    response.statusCode = 500;
    response.end('Offline fixture failed');
  }
});
await new Promise((resolve) => api.listen(0, '127.0.0.1', resolve));
let h;
try {
  h = await nativeHarness('ollama-shared', {
    appEnv: { OLLAMA_HOST: `http://127.0.0.1:${api.address().port}` },
  });
  const owner = await h.invoke('add_account', { name: 'Original Claude', provider: 'claude' });
  const local = await h.invoke('add_account', { name: 'Local inference', provider: 'ollama' });
  const profile = path.join(h.data, 'accounts', owner.id);
  const pid = path.join(h.data, 'mcp.pid');
  await mkdir(profile, { recursive: true });
  await writeFile(
    path.join(profile, '.claude.json'),
    JSON.stringify({
      mcpServers: {
        original: {
          command: process.execPath,
          args: [path.join(h.root, 'tests/fixtures/local-tools.mjs')],
          env: { OWNER_MARKER: 'claude-original', PID_FILE: pid },
        },
      },
    }),
  );
  async function start(title, model, text = 'Use the shared integration fixture') {
    const session = await h.invoke('create_session', { workspace: h.workspace });
    await h.invoke('update_session', { id: session.id, title, pinned: false });
    await h.invoke('send_message', {
      id: session.id,
      text,
      options: { mode: 'ollama', accountId: local.id, model, effort: '', accessMode: 'autopilot' },
    });
    await h.page.reload();
    await h.page.locator('.session-row').filter({ hasText: title }).click();
    return session;
  }
  async function complete(session) {
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
    assert(messages.at(-1).text.includes('LOCAL_SHARED_VERIFIED'), JSON.stringify(messages.at(-1)));
    await expect
      .poll(async () => {
        try {
          process.kill(Number(await readFile(pid, 'utf8')), 0);
          return false;
        } catch {
          return true;
        }
      })
      .toBe(true);
    return messages;
  }
  const native = await start('Local native shared MCP', 'local-fixture');
  const form = h.page.getByRole('form', { name: 'Request from a connector' });
  await expect(form).toBeVisible({ timeout: 20000 });
  await form.getByRole('radio', { name: 'Accept', exact: true }).check();
  await form.getByRole('radio', { name: /^true\b/ }).check();
  await form.getByRole('button', { name: 'Send answers' }).click();
  const messages = await complete(native);
  assert(
    messages
      .at(-1)
      .activity.some(
        (a) =>
          a.kind === 'interaction-result' && Object.keys(JSON.parse(a.text).answers).length === 0,
      ),
  );
  await complete(await start('Local JSON shared MCP', 'schema-fixture'));
  const gpt = await h.invoke('add_account', { name: 'Owning ChatGPT', provider: 'gpt' });
  ownerId = gpt.id;
  const gptProfile = path.join(h.data, 'accounts', gpt.id);
  await writeFile(path.join(gptProfile, 'auth.json'), '{}');
  await writeFile(path.join(gptProfile, 'config.toml'), '');
  await writeFile(path.join(gptProfile, 'native-gateway-fixture'), 'offline');
  await complete(await start('Local calls owning ChatGPT', 'local-fixture', 'USE_OWNER_GATEWAY'));
  const nativeMethods = await readFile(path.join(gptProfile, 'native-rpc.txt'), 'utf8');
  assert(nativeMethods.includes('mcpServer/tool/call'));
  assert(!nativeMethods.includes('turn/start'));
  await writeFile(
    path.join(h.workspace, '.mcp.json'),
    JSON.stringify({
      mcpServers: {
        httpOriginal: {
          type: 'http',
          url: `http://127.0.0.1:${api.address().port}/mcp`,
          headers: { authorization: 'Bearer localhost-fixture' },
        },
      },
    }),
  );
  await complete(await start('Local HTTP shared MCP', 'local-fixture', 'USE_HTTP_SHARED'));
  assert(httpClosed > 0, 'Turn completion must close the HTTP MCP session');
  assert.deepEqual(failures, []);
  console.log(
    JSON.stringify(
      {
        checks: [
          'Ollama native tools call an original Claude MCP with private host consent',
          'Ollama schema actions consume the original typed MCP result',
          'Ollama calls the owning ChatGPT gateway through direct RPC without starting GPT inference',
          'Turn completions clean up the original service process',
          'Ollama consumes an authenticated HTTP MCP result and closes its original session',
        ],
        liveCalls: 0,
      },
      null,
      2,
    ),
  );
} finally {
  await h?.close();
  api.closeAllConnections();
  await new Promise((resolve) => api.close(resolve));
}
