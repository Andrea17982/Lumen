// Minimal MCP stdio server with no dependencies. It declares no permissions, so Lumen starts it
// in this folder with a scrubbed environment and no project path.
import { createInterface } from 'node:readline';

const send = (frame) => process.stdout.write(JSON.stringify({ jsonrpc: '2.0', ...frame }) + '\n');
const tool = {
  name: 'count_words',
  description: 'Count words, lines and characters in a piece of text.',
  inputSchema: {
    type: 'object',
    properties: { text: { type: 'string' } },
    required: ['text'],
  },
  annotations: { readOnlyHint: true },
};

createInterface({ input: process.stdin }).on('line', (line) => {
  let message;
  try {
    message = JSON.parse(line);
  } catch {
    return;
  }
  const { id, method, params } = message;
  if (id === undefined) return;
  if (method === 'initialize')
    send({
      id,
      result: {
        protocolVersion: params?.protocolVersion || '2025-06-18',
        capabilities: { tools: {} },
        serverInfo: { name: 'word-count', version: '1.0.0' },
      },
    });
  else if (method === 'tools/list') send({ id, result: { tools: [tool] } });
  else if (method === 'tools/call' && params?.name === 'count_words') {
    const text = String(params.arguments?.text ?? '');
    const counts = {
      words: text.split(/\s+/).filter(Boolean).length,
      lines: text ? text.split(/\r?\n/).length : 0,
      characters: [...text].length,
    };
    send({ id, result: { content: [{ type: 'text', text: JSON.stringify(counts) }] } });
  } else send({ id, error: { code: -32601, message: `Unknown method ${method}` } });
});
