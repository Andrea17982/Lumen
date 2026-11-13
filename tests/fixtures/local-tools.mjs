// Offline MCP peer for the local-inference client: no model, network, credentials or shell.
import { createInterface } from 'node:readline';
import { appendFileSync, writeFileSync } from 'node:fs';
if (process.env.PID_FILE) writeFileSync(process.env.PID_FILE, String(process.pid));
const send = (frame) => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', ...frame }) + '\n');
const result = (id, value) => send({ id, result: value });
let pending;
let revised = false;
for await (const line of createInterface({ input: process.stdin })) {
  const message = JSON.parse(line);
  if (message.method === 'initialize') {
    if (!message.params.capabilities.elicitation) throw new Error('Missing elicitation capability');
    result(message.id, {
      protocolVersion: '2025-06-18',
      capabilities: {
        tools: { listChanged: true },
        ...(process.env.RESOURCE_MODE ? { resources: {}, prompts: {} } : {}),
      },
      serverInfo: { name: 'local-tools', version: '1' },
    });
  }
  if (message.method === 'tools/list') {
    if (!message.params.cursor) {
      result(message.id, {
        tools: [
          {
            name: revised ? 'new_echo' : 'echo',
            description: 'Local fixture echo',
            inputSchema: { type: 'object' },
            annotations: { readOnlyHint: true },
          },
        ],
        nextCursor: 'second',
      });
    } else {
      result(message.id, {
        tools: [
          { name: 'write', inputSchema: { type: 'object' }, annotations: { readOnlyHint: false } },
        ],
      });
    }
  }
  if (message.method === 'resources/list')
    result(message.id, {
      resources: [{ uri: 'fixture://original', name: 'Original account resource' }],
      ...(message.params.cursor ? {} : { nextCursor: 'resource-second' }),
    });
  if (message.method === 'resources/templates/list')
    result(message.id, {
      resourceTemplates: [{ uriTemplate: 'fixture://{name}', name: 'Account template' }],
    });
  if (message.method === 'resources/read')
    result(message.id, { contents: [{ uri: message.params.uri, text: 'Original resource body' }] });
  if (message.method === 'prompts/list')
    result(message.id, {
      prompts: [{ name: 'review', arguments: [{ name: 'subject', required: true }] }],
    });
  if (message.method === 'prompts/get')
    result(message.id, {
      messages: [
        {
          role: 'user',
          content: { type: 'text', text: 'Review ' + message.params.arguments.subject },
        },
      ],
    });
  if (message.method === 'tools/call') {
    if (message.params.arguments.refresh) {
      revised = true;
      send({ method: 'notifications/tools/list_changed' });
    }
    if (process.env.CALL_FILE) appendFileSync(process.env.CALL_FILE, message.params.name + '\n');
    if (message.params.arguments.wait) continue;
    if (message.params.arguments.malformed) {
      process.stdout.write('{private-invalid-frame\n');
      continue;
    }
    if (
      message.params.arguments.elicit ||
      message.params.arguments.sample ||
      message.params.arguments.withdraw
    ) {
      pending = message.id;
      if (message.params.arguments.sample)
        send({ id: 'nested', method: 'sampling/createMessage', params: {} });
      else
        send({
          id: 'nested',
          method: 'elicitation/create',
          params: {
            message: 'Confirm local fixture',
            requestedSchema: {
              type: 'object',
              properties: { approved: { type: 'boolean' } },
              required: ['approved'],
            },
          },
        });
      if (message.params.arguments.withdraw)
        setTimeout(
          () => send({ method: 'notifications/cancelled', params: { requestId: 'nested' } }),
          50,
        );
      continue;
    }
    result(message.id, {
      content: [
        {
          type: 'text',
          text: JSON.stringify({
            owner: process.env.OWNER_MARKER,
            arguments: message.params.arguments,
          }),
        },
      ],
      structuredContent: { preserved: true },
    });
  }
  if (message.id === 'nested' && !message.method) {
    result(pending, {
      content: [
        {
          type: 'text',
          text: message.error
            ? 'Sampling unsupported; no inference'
            : `Consent ${message.result.action}`,
        },
      ],
    });
    pending = undefined;
  }
}
