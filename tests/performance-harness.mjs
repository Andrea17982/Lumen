import { chromium } from '@playwright/test';
import { spawn, execFileSync } from 'node:child_process';
import { mkdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { createServer } from 'node:net';
import assert from 'node:assert/strict';

export async function performanceHarness(executable) {
  const root = process.cwd();
  const artifacts = path.resolve(process.env.LUMEN_PERF_ROOT || '.impeccable/performance');
  const config = JSON.parse(await readFile(path.join(artifacts, 'config.json'), 'utf8'));
  const data = path.join(process.env.APPDATA, config.identifier);
  const workspace = path.join(artifacts, 'workspace');
  await mkdir(workspace, { recursive: true });
  const server = createServer();
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  const port = server.address().port;
  await new Promise((r) => server.close(r));
  const start = performance.now();
  const app = spawn(executable, [], {
    windowsHide: true,
    stdio: 'pipe',
    env: {
      ...process.env,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
    },
  });
  app.stdout.resume();
  app.stderr.resume();
  let browser;
  try {
    for (let i = 0; i < 150; i++) {
      try {
        browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
        break;
      } catch {
        await new Promise((r) => setTimeout(r, 100));
      }
    }
    assert(browser, 'No isolated release WebView2');
    const page = browser
      .contexts()[0]
      .pages()
      .find((p) => !p.url().startsWith('devtools:'));
    await page.locator('h1').waitFor();
    const startupMs = performance.now() - start;
    const invoke = (command, args = {}) =>
      page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), {
        command,
        args,
      });
    assert.equal((await invoke('diagnostics')).dataDir.toLowerCase(), data.toLowerCase());
    assert.equal((await invoke('list_accounts')).length, 0);
    await page.emulateMedia({ reducedMotion: 'reduce' });
    const cdp = await browser.newBrowserCDPSession();
    const renderer = await page.context().newCDPSession(page);
    await renderer.send('Performance.enable');
    return {
      root,
      data,
      artifacts,
      workspace,
      page,
      invoke,
      renderer,
      cdp,
      pid: app.pid,
      startupMs,
      async close() {
        await browser.close().catch(() => {});
        if (app.exitCode === null) app.kill();
        app.stdin.destroy();
        app.stdout.destroy();
        app.stderr.destroy();
        app.unref();
      },
    };
  } catch (error) {
    if (app.exitCode === null) app.kill();
    await browser?.close().catch(() => {});
    throw error;
  }
}

export function privateProcesses(pid) {
  // Only this process tree, using PrivateMemorySize64 rather than shared working sets.
  const script = `$all=Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId; $ids=[System.Collections.Generic.HashSet[int]]::new(); [void]$ids.Add(${pid}); do {$added=$false; foreach($p in $all){if($ids.Contains([int]$p.ParentProcessId) -and $ids.Add([int]$p.ProcessId)){$added=$true}}}while($added); $rows=@(foreach($id in $ids){$p=Get-Process -Id $id -ErrorAction SilentlyContinue;if($p){[pscustomobject]@{pid=$id;privateBytes=$p.PrivateMemorySize64}}}); ConvertTo-Json -Compress -InputObject $rows`;
  const rows = JSON.parse(
    execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', script], {
      encoding: 'utf8',
      windowsHide: true,
    }),
  );
  return { count: rows.length, privateBytes: rows.reduce((n, r) => n + r.privateBytes, 0) };
}
