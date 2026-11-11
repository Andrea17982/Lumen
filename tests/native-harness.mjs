// Isolated native fixture plumbing. Run serially; offline by default, live callers opt out.
import { chromium } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createServer } from 'node:net';
import assert from 'node:assert/strict';

export async function nativeHarness(name, { executable, offline = true } = {}) {
  const root = process.cwd();
  const data = await mkdtemp(path.join(tmpdir(), `lumen-${name}-`));
  const workspace = path.join(data, 'workspace');
  await mkdir(workspace);
  const server = createServer();
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const port = server.address().port;
  await new Promise((resolve) => server.close(resolve));
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  let vite;
  let browser;
  let app;
  const close = async () => {
    await browser?.close().catch(() => {});
    if (app?.exitCode === null) app.kill();
    if (vite?.exitCode === null) vite.kill();
    // A CLI descendant can inherit a pipe after its owned parent exits. Closing these
    // handles prevents a concluded multi-window experiment keeping Node alive.
    for (const child of [app, vite]) {
      child?.stdin?.destroy();
      child?.stdout?.destroy();
      child?.stderr?.destroy();
      child?.unref();
    }
  };
  try {
    try {
      await fetch('http://127.0.0.1:1420');
    } catch {
      vite = spawn(process.execPath, ['node_modules/vite/bin/vite.js', '--host', '127.0.0.1'], {
        cwd: root,
        windowsHide: true,
        stdio: 'pipe',
      });
      for (let i = 0; i < 100; i++) {
        try {
          await fetch('http://127.0.0.1:1420');
          break;
        } catch {
          await sleep(100);
        }
      }
    }
    app = spawn(executable || path.join(root, 'src-tauri/target/debug/lumen.exe'), [], {
      cwd: root,
      windowsHide: true,
      stdio: 'pipe',
      env: {
        ...process.env,
        LUMEN_TEST_DATA_DIR: data,
        WEBVIEW2_USER_DATA_FOLDER: path.join(data, 'webview'),
        WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
        PATH: offline
          ? `${path.join(root, 'tests/fixtures/cli')};${process.env.PATH}`
          : process.env.PATH,
      },
    });
    let errors = '';
    app.stderr.on('data', (chunk) => {
      errors += chunk;
    });
    for (let i = 0; i < 120; i++) {
      try {
        browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
        break;
      } catch {
        await sleep(150);
      }
    }
    assert(browser, `No isolated WebView2: ${errors}`);
    const page = browser
      .contexts()[0]
      .pages()
      .find((p) => !p.url().startsWith('devtools:'));
    assert(page);
    await page.locator('h1').waitFor();
    const invoke = (command, args = {}) =>
      page.evaluate(({ command, args }) => window.__TAURI_INTERNALS__.invoke(command, args), {
        command,
        args,
      });
    assert.equal((await invoke('diagnostics')).dataDir, data);
    return { root, data, workspace, page, invoke, close, pid: app.pid };
  } catch (error) {
    await close();
    throw error;
  }
}
