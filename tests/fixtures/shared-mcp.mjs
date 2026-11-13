// Original service fixture; no provider, network or account access.
import { createInterface } from 'node:readline';
import { writeFileSync } from 'node:fs';
import { spawn } from 'node:child_process';
if (process.env.STARTUP_DELAY_MS)
  await new Promise((resolve) => setTimeout(resolve, Number(process.env.STARTUP_DELAY_MS)));
if (process.env.FIXTURE_PID_FILE) writeFileSync(process.env.FIXTURE_PID_FILE, String(process.pid));
const output = (id, result) =>
  process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id, result }) + '\n');
for await (const line of createInterface({ input: process.stdin })) {
  const message = JSON.parse(line);
  if (message.method === 'initialize')
    output(message.id, {
      protocolVersion: '2025-06-18',
      serverInfo: { name: 'original-service', version: '1' },
      capabilities: { tools: {} },
    });
  if (message.method === 'tools/list')
    output(message.id, {
      tools: [{ name: 'echo', description: 'Original service', inputSchema: { type: 'object' } }],
    });
  if (message.method === 'tools/call' && message.params.arguments?.orphan) {
    const descendant = spawn(process.execPath, ['-e', 'setTimeout(()=>{},60000)'], {
      windowsHide: true,
      stdio: 'ignore',
    });
    process.stdout.write(
      JSON.stringify({
        jsonrpc: '2.0',
        id: message.id,
        result: { content: [{ type: 'text', text: String(descendant.pid) }] },
      }) + '\n',
      () => process.exit(0),
    );
    continue;
  }
  if (message.method === 'tools/call')
    output(message.id, {
      content: [
        {
          type: 'text',
          text: JSON.stringify({
            owner: process.env.OWNER_MARKER,
            arguments: message.params.arguments,
          }),
        },
      ],
    });
}
