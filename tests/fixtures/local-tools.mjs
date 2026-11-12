// Offline MCP peer for the local-inference client: no model, network, credentials or shell.
import { createInterface } from 'node:readline';
import { appendFileSync, writeFileSync } from 'node:fs';
if (process.env.PID_FILE) writeFileSync(process.env.PID_FILE, String(process.pid));
const send = (frame) => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', ...frame }) + '\n');
const result = (id, value) => send({ id, result: value });
let pending;
for await (const line of createInterface({ input: process.stdin })) {
  const message = JSON.parse(line);
  if (message.method === 'initialize') {
    if (!message.params.capabilities.elicitation) throw new Error('Missing elicitation capability');
    result(message.id, {
      protocolVersion: '2025-06-18',
      capabilities: { tools: {} },
      serverInfo: { name: 'local-tools', version: '1' },
    });
  }
  if (message.method === 'tools/list') {
    if (!message.params.cursor) {
      result(message.id, {
        tools: [
          {
            name: 'echo',
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
  if (message.method === 'tools/call') {
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
