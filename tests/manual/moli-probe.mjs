// Explicit local feasibility probe; no installation, account cookies or external service.
import { chromium } from 'playwright';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { mkdir, writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
const executable = process.env.LUMEN_MOLI_EXE;
if (!executable) throw Error('Set LUMEN_MOLI_EXE to an inspected official Moli binary.');
const fixture = createServer((req, res) => {
  res.setHeader('Content-Type', 'text/html');
  res.end(
    '<!doctype html><html><head><title>Agent browser probe</title></head><body><h1 id="ready">Waiting</h1><input aria-label="Name"><button id="action" onclick="document.querySelector(\'h1\').textContent=\'Clicked\'">Continue</button><script>document.querySelector(\'h1\').textContent="JS ready"</script></body></html>',
  );
});
await new Promise((resolve) => fixture.listen(0, '127.0.0.1', resolve));
const url = `http://127.0.0.1:${fixture.address().port}/`;
const median = (values) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
const results = [];
try {
  for (const engine of ['moli-dom', 'moli-layout', 'chromium']) {
    let process;
    let browser;
    const start = performance.now();
    try {
      if (engine === 'chromium') browser = await chromium.launch({ headless: true });
      else {
        const port = 19380 + (engine === 'moli-layout' ? 1 : 0);
        process = spawn(
          executable,
          [
            'serve',
            '--host',
            '127.0.0.1',
            '--port',
            String(port),
            ...(engine === 'moli-layout' ? ['--layout'] : []),
          ],
          { windowsHide: true, stdio: 'ignore' },
        );
        for (let i = 0; i < 100; i++) {
          if (process.exitCode !== null) throw Error('Moli exited before CDP readiness');
          try {
            await fetch(`http://127.0.0.1:${port}/json/version`);
            break;
          } catch {
            await new Promise((resolve) => setTimeout(resolve, 10));
          }
        }
        browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
      }
      const readyMs = performance.now() - start;
      const context = engine === 'chromium' ? await browser.newContext() : browser.contexts()[0];
      const times = [];
      for (let i = 0; i < 12; i++) {
        const t = performance.now();
        const page = await context.newPage();
        await page.goto(url, { waitUntil: 'domcontentloaded' });
        assert.equal(await page.locator('h1').textContent(), 'JS ready');
        await page.locator('input').fill('Lumen');
        assert.equal(await page.locator('input').inputValue(), 'Lumen');
        await page.locator('#action').evaluate((button) => button.click());
        assert.equal(await page.locator('h1').textContent(), 'Clicked');
        times.push(performance.now() - t);
        if (i === 0 && engine !== 'moli-dom') {
          await mkdir('.impeccable/review', { recursive: true });
          await page.screenshot({ path: `.impeccable/review/${engine}-probe.png` });
        }
        await page.close();
      }
      results.push({
        engine,
        readyMs,
        episodeP50Ms: median(times),
        episodes: times.length,
        passed: true,
      });
    } catch (error) {
      results.push({ engine, passed: false, error: String(error) });
    } finally {
      if (browser) await browser.close().catch(() => {});
      if (process && process.exitCode === null) process.kill();
    }
  }
} finally {
  fixture.close();
}
await mkdir('.impeccable/review', { recursive: true });
await writeFile(
  '.impeccable/review/moli-performance.json',
  JSON.stringify(
    {
      note: 'Local synthetic DOM/form workload, single startup sample per engine; no claim of web-wide compatibility or inference speedup.',
      results,
    },
    null,
    2,
  ),
);
console.log(JSON.stringify(results, null, 2));
assert(
  results.every((r) => r.passed),
  'A browser feasibility probe failed',
);
