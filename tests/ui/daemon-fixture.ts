// A real daemon with an isolated data directory and the offline CLI fixtures, reachable from the
// page through the test bridge. Each caller owns the daemon's lifetime and must call `close`.
import { expect } from '@playwright/test';
import { existsSync } from 'node:fs';
import { mkdir, mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { daemonClient } from '../daemon-harness.mjs';
import { startBridge } from '../support/ipc-bridge.mjs';

export async function startDaemon(origin: string, { welcome = false } = {}) {
  const executable = path.resolve('src-tauri/target/debug/lumen-daemon.exe');
  expect(existsSync(executable), 'build the daemon first: cargo build -p lumen-daemon').toBe(true);
  const data = await mkdtemp(path.join(tmpdir(), 'lumen-ui-'));
  const workspace = path.join(data, 'workspace');
  await mkdir(workspace);
  const client = await daemonClient(data, executable, { fixturePathOnly: welcome });
  let daemon = client.start();
  await client.ready();
  // Conversations are listed by project: the workspace is the project the UI opens.
  if (!welcome) {
    await client.call('onboarding_state', { action: 'skipSetup' });
    await client.call('add_workspace', { name: 'Fixture', folder: workspace });
  }
  const bridge = await startBridge(client, { origin });
  const stopped = () => new Promise((resolve) => (daemon.exitCode !== null || daemon.signalCode !== null ? resolve(null) : daemon.once('exit', resolve)));
  return {
    client,
    data,
    workspace,
    get daemon() {
      return daemon;
    },
    url: (query: Record<string, string> = {}) => `/?${new URLSearchParams({ locale: 'en', bridge: bridge.url, token: bridge.token, ...query })}`,
    /** Stops the service cleanly, for example to seed its store. */
    async stop() {
      await client.call('__shutdown');
      await stopped();
    },
    async restart() {
      daemon = client.start();
      await client.ready();
    },
    async close() {
      await bridge.close();
      await client.call('__shutdown').catch(() => {});
      if (daemon.exitCode === null) daemon.kill();
      await stopped();
      await rm(data, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
    },
  };
}
