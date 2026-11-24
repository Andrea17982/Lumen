// Manual screen reader check (opt-in): opens the debug app on a seeded 5,000-message session in an
// isolated data directory and waits until you close the window. Offline; no provider is called.
// Build first: npx tauri build --debug --no-bundle --features webdriver --config src-tauri/desktop/tauri.conf.json
import { mkdir, mkdtemp, rm } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { daemonClient, sleep } from '../daemon-harness.mjs';
import { appPath, daemonPath, requireBuilds } from '../support/app.mjs';
import { seed, transcript } from '../support/transcript.mjs';

requireBuilds();
const data = await mkdtemp(path.join(tmpdir(), 'lumen-manual-'));
const workspace = path.join(data, 'workspace');
await mkdir(workspace);
const client = await daemonClient(data, daemonPath);
const daemon = client.start();
await client.ready();
await client.call('add_workspace', { name: 'Fixture', folder: workspace });
const chat = await client.call('create_session', { workspace });
await client.call('__shutdown');
await new Promise((resolve) => (daemon.exitCode !== null ? resolve() : daemon.once('exit', resolve)));
seed(data, chat.id, transcript(5000, 5));
console.log('Opening Lumen with a 5,000-message conversation. Close the window to finish.');
const app = spawn(appPath, [], { stdio: 'ignore', env: { ...process.env, LUMEN_TEST_DATA_DIR: data } });
await new Promise((resolve) => app.once('exit', resolve));
await client.call('__shutdown').catch(() => {});
await sleep(500);
await rm(data, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
console.log('Done; the test data was removed.');
