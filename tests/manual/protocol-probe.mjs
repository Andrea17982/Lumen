// Installed-CLI metadata only: no prompt, account read, login or model request.
import { spawn, execFileSync } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
const config = await mkdtemp(path.join(tmpdir(), 'lumen-protocol-probe-'));
const executable =
  process.platform === 'win32'
    ? path.join(
        execFileSync('npm.cmd', ['root', '-g'], {
          shell: true,
          windowsHide: true,
          encoding: 'utf8',
        }).trim(),
        '@anthropic-ai',
        'claude-code',
        'bin',
        'claude.exe',
      )
    : 'claude';
const started = performance.now();
const child = spawn(
  executable,
  [
    '--print',
    '--verbose',
    '--input-format',
    'stream-json',
    '--output-format',
    'stream-json',
    '--permission-prompt-tool',
    'stdio',
    '--tools',
    '',
    '--session-id',
    randomUUID(),
  ],
  {
    windowsHide: true,
    stdio: 'pipe',
    env: {
      ...process.env,
      CLAUDE_CONFIG_DIR: config,
      ANTHROPIC_API_KEY: '',
      ANTHROPIC_AUTH_TOKEN: '',
      CLAUDE_CODE_OAUTH_TOKEN: '',
    },
  },
);
let buffer = '',
  finished = false;
const finish = (supported, reason) => {
  if (finished) return;
  finished = true;
  console.log(
    JSON.stringify({
      provider: 'claude',
      protocol: 'stream-json stdio',
      initialized: supported,
      elapsedMs: Math.round(performance.now() - started),
      reason,
    }),
  );
  child.kill();
  process.exitCode = supported ? 0 : 1;
};
const timer = setTimeout(() => finish(false, 'Initialization timed out'), 30000);
child.on('error', (e) => finish(false, e.message));
child.stdout.on('data', (chunk) => {
  buffer += chunk.toString();
  let nl;
  while ((nl = buffer.indexOf('\n')) >= 0) {
    const line = buffer.slice(0, nl);
    buffer = buffer.slice(nl + 1);
    try {
      const v = JSON.parse(line);
      if (v.type === 'control_response' && v.response?.request_id === 'probe')
        finish(v.response.subtype === 'success', v.response.subtype);
    } catch {
      /* Other CLI output is not persisted. */
    }
  }
});
child.stderr.resume();
child.on('exit', () => {
  clearTimeout(timer);
  if (!finished) finish(false, 'CLI exited');
});
child.stdin.write(
  JSON.stringify({
    type: 'control_request',
    request_id: 'probe',
    request: { subtype: 'initialize', forwardSubagentText: true },
  }) + '\n',
);
await new Promise((resolve) => child.on('close', resolve));
await rm(config, { recursive: true, force: true });
