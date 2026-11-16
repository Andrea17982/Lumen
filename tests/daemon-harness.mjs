import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
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
    let buffered = Buffer.alloc(0);
    const messages = [],
      waiting = [];
    let ended;
    io.on('data', (chunk) => {
      buffered = Buffer.concat([buffered, chunk]);
      while (buffered.length >= 4 && buffered.length >= buffered.readUInt32BE(0) + 4) {
        const length = buffered.readUInt32BE(0);
        const value = JSON.parse(buffered.subarray(4, length + 4));
        buffered = buffered.subarray(length + 4);
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
    const bytes = Buffer.from(JSON.stringify({ protocol: 1, build, command, args, ...override }));
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
            ? `${path.resolve('tests/fixtures/cli')};${process.env.PATH}`
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
