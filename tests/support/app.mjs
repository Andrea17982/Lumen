// Launches the debug desktop app built with the `webdriver` feature against an isolated data
// directory and the offline CLI fixtures, and connects the minimal WebDriver client to it.
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { createServer } from 'node:net';
import path from 'node:path';
import { webdriver } from './webdriver.mjs';

export const appPath = path.resolve('src-tauri/desktop/target/debug/Lumen.exe');
export const daemonPath = path.resolve('src-tauri/target/debug/lumen-daemon.exe');

export function requireBuilds() {
  for (const file of [appPath, daemonPath]) assert(existsSync(file), `missing ${file}: see the commands in CLAUDE.md`);
}

const freePort = () =>
  new Promise((resolve) => {
    const server = createServer().listen(0, '127.0.0.1', () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
  });

export async function launchApp(data) {
  const port = await freePort();
  const shell = spawn(appPath, [], {
    stdio: 'ignore',
    env: {
      ...process.env,
      LUMEN_TEST_DATA_DIR: data,
      TAURI_WEBDRIVER_PORT: String(port),
      PATH: `${path.resolve('tests/fixtures/cli')};${process.env.PATH}`,
    },
  });
  const browser = await webdriver(port);
  return {
    browser,
    pid: shell.pid,
    async close() {
      await browser.close();
      shell.kill();
      await new Promise((resolve) => (shell.exitCode !== null ? resolve() : shell.once('exit', resolve)));
    },
  };
}

export const alive = (pid) => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};
