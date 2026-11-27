import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { readFile, realpath } from 'node:fs/promises';
import net from 'node:net';
import path from 'node:path';
import { tmpdir } from 'node:os';

export const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
export async function daemonClient(
  data,
  executable = path.resolve(
    `src-tauri/target/debug/lumen-daemon${process.platform === 'win32' ? '.exe' : ''}`,
  ),
  { fixturePathOnly = false } = {},
) {
  const canonical = await realpath(data);
  const hash = createHash('sha256').update(canonical).digest('hex').slice(0, 32);
  const endpoint =
    process.platform === 'win32'
      ? `\\\\.\\pipe\\lumen-${hash}`
      : path.join(tmpdir(), `lumen-${process.getuid()}`, `${hash}.sock`);
  const build = createHash('sha256')
    .update(await readFile(executable))
    .digest('hex');
  async function open(command, args = {}, override = {}) {
    const io = net.createConnection(endpoint);
    // Chunks are joined once per complete frame: re-joining on every chunk is quadratic on large replies.
    let chunks = [];
    let size = 0;
    const messages = [],
      waiting = [];
    let ended;
    io.on('data', (chunk) => {
      chunks.push(chunk);
      size += chunk.length;
      while (size >= 4) {
        if (chunks[0].length < 4) chunks = [Buffer.concat(chunks, size)];
        const length = chunks[0].readUInt32BE(0);
        if (size < length + 4) break;
        const frame = chunks.length === 1 ? chunks[0] : Buffer.concat(chunks, size);
        const value = JSON.parse(frame.subarray(4, length + 4));
        const rest = frame.subarray(length + 4);
        chunks = rest.length ? [rest] : [];
        size = rest.length;
        const waiter = waiting.shift();
        if (waiter) waiter.resolve(value);
        else messages.push(value);
      }
    });
    const failed = (error) => {
      ended = error;
      for (const waiter of waiting.splice(0)) waiter.reject(error);
    };
    io.on('error', failed);
    io.on('close', () => failed(new Error('IPC closed')));
    await new Promise((resolve, reject) => {
      io.once('connect', resolve);
      io.once('error', reject);
    });
    const bytes = Buffer.from(
      JSON.stringify({ protocol: 2, build, command, args, command_id: randomUUID(), ...override }),
    );
    const header = Buffer.alloc(4);
    header.writeUInt32BE(bytes.length);
    io.write(Buffer.concat([header, bytes]));
    return {
      io,
      next: () =>
        messages.length
          ? Promise.resolve(messages.shift())
          : ended
            ? Promise.reject(ended)
            : new Promise((resolve, reject) => waiting.push({ resolve, reject })),
    };
  }
  async function call(command, args = {}, override = {}) {
    const peer = await open(command, args, override);
    try {
      const reply = await peer.next();
      if ('Err' in reply.result) throw new Error(reply.result.Err);
      return reply.result.Ok;
    } finally {
      peer.io.destroy();
    }
  }
  const start = () =>
    spawn(executable, ['--data-dir', data, '--resources', path.resolve('src-tauri')], {
      windowsHide: true,
      stdio: 'pipe',
      env: {
        ...process.env,
        LUMEN_TEST_DATA_DIR: data,
        PATH:
          process.platform === 'win32'
            ? [path.resolve('tests/fixtures/cli'), ...(fixturePathOnly
                ? [path.join(process.env.SystemRoot, 'System32'), path.join(process.env.SystemRoot, 'System32', 'WindowsPowerShell', 'v1.0'), path.dirname(process.execPath)]
                : [process.env.PATH])].join(';')
            : process.env.PATH,
      },
    });
  async function ready() {
    for (let n = 0; n < 150; n++) {
      try {
        return await call('__status');
      } catch {
        await sleep(100);
      }
    }
    throw new Error('Daemon startup timed out');
  }
  return { call, open, start, ready, endpoint, build, executable };
}
