// Explicit live-account metadata only: no inference, isolated data and bounded Windows telemetry.
import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { readFile, writeFile, mkdir, unlink } from 'node:fs/promises';
import path from 'node:path';
import { nativeHarness } from './native-harness.mjs';
import { accountMetadata, attachLiveAccounts } from './quality-live-accounts.mjs';
assert.equal(process.env.LUMEN_LIVE_IDLE, '1');
const out = path.resolve('.codex/verification');
await mkdir(out, { recursive: true });
const before = () =>
  execFileSync(
    'powershell.exe',
    [
      '-NoProfile',
      '-NonInteractive',
      '-Command',
      '$v=(Get-ItemProperty -LiteralPath HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Run -Name Lumen -ErrorAction SilentlyContinue).Lumen; ConvertTo-Json -Compress -InputObject $v',
    ],
    { encoding: 'utf8', windowsHide: true },
  ).trim();
const loginBefore = before();
const configs = [];
const metadata = accountMetadata();
for (const a of metadata.accounts)
  for (const name of [
    'config.toml',
    'settings.json',
    '.gemini/config/mcp_config.json',
    '.gemini/settings.json',
  ]) {
    const file = path.join(metadata.root, 'accounts', a.id, name);
    try {
      configs.push({ file, bytes: await readFile(file) });
    } catch (e) {
      if (e.code !== 'ENOENT') throw e;
      configs.push({ file, bytes: null });
    }
  }
let h, homes, probe;
const report = { startedAt: new Date().toISOString(), phases: [], noInference: true };
const flush = () => writeFile(path.join(out, 'idle-current.json'), JSON.stringify(report, null, 2));
try {
  h = await nativeHarness('idle-acceptance', { offline: false, appEnv: { NO_BROWSER: 'true' } });
  report.pid = h.pid;
  report.data = h.data;
  const prefs = await h.invoke('get_settings');
  await h.invoke('save_settings', {
    value: { ...prefs, openAtLogin: true, closeToTray: true, trayHintShown: true },
  });
  assert.equal(before(), loginBefore, 'Isolated fixture changed Windows autostart');
  await h.invoke('save_settings', {
    value: { ...prefs, openAtLogin: false, closeToTray: true, trayHintShown: true },
  });
  report.autostartIsolationPassed = before() === loginBefore;
  homes = await attachLiveAccounts(h, ['claude', 'gpt', 'antigravity']);
  report.accounts = homes.accounts.map((a) => ({ provider: a.provider, ready: a.ready }));
  assert(homes.accounts.every((a) => a.ready));
  probe = spawn(
    'powershell.exe',
    [
      '-NoProfile',
      '-ExecutionPolicy',
      'Bypass',
      '-File',
      path.join(out, 'window-probe.ps1'),
      '-RootPid',
      String(h.pid),
      '-Output',
      path.join(out, 'window-events.json'),
      '-Seconds',
      '1110',
    ],
    { windowsHide: true, stdio: 'pipe' },
  );
  let errors = '';
  probe.stderr.on('data', (c) => (errors += c));
  probe.stdout.resume();
  await h.page.reload();
  await h.page.locator('h1').waitFor();
  for (const phase of ['foreground', 'background', 'tray']) {
    if (phase === 'tray') await h.invoke('plugin:window|close', { label: 'main' });
    report.phases.push({ phase, startedAt: new Date().toISOString() });
    await flush();
    for (let i = 0; i < 36; i++) await new Promise((r) => setTimeout(r, 10000));
    report.phases.at(-1).finishedAt = new Date().toISOString();
    await flush();
  }
  while (probe.exitCode === null) await new Promise((r) => setTimeout(r, 1000));
  assert.equal(probe.exitCode, 0, errors);
  report.events = JSON.parse(await readFile(path.join(out, 'window-events.json'), 'utf8')) || [];
  assert(
    report.events.some((e) => e.owned),
    'Window hook positive control missing',
  );
  report.ownedConsoleWindows = report.events.filter(
    (e) => e.owned && /console|cascadia/i.test(e.windowClass || ''),
  );
  report.finishedAt = new Date().toISOString();
} finally {
  if (probe?.exitCode === null) probe.kill();
  await h?.close();
  for (const { file, bytes } of configs) {
    if (bytes) {
      await mkdir(path.dirname(file), { recursive: true });
      await writeFile(file, bytes);
    } else
      await unlink(file).catch((e) => {
        if (e.code !== 'ENOENT') throw e;
      });
  }
  await homes?.cleanup();
  report.configsRestored = true;
  await flush();
}
assert.equal(report.ownedConsoleWindows.length, 0);
console.log(
  'Passed: 18 minutes, three idle phases, real account metadata, no Lumen console windows; isolated autostart unchanged.',
);
