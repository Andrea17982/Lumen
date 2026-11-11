// Focused A/B/A/B investigation after the nonstationary full-workload measurements.
import { readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { performanceHarness } from './performance-harness.mjs';
const root = '.impeccable/performance/';
const session = JSON.parse(await readFile(root + 'sessions.json', 'utf8')).find(
  (s) => s.size === 5000 && s.kind === 'mixed',
);
const results = [];
for (const arm of ['baseline', 'candidate', 'baseline', 'candidate']) {
  const h = await performanceHarness(path.resolve(root + arm + '/lumen.exe'));
  try {
    await h.page.locator('.session-row').filter({ hasText: session.title }).click();
    await h.page.waitForFunction(
      () =>
        document.querySelectorAll('.message').length === 5000 &&
        document.querySelectorAll('.prose pre').length === 2500,
    );
    await h.page.evaluate(
      () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))),
    );
    const html = await h.page.locator('.message-list').evaluate((e) => e.innerHTML);
    const windowState = await h.page.evaluate(() => ({
      focused: document.hasFocus(),
      visibility: document.visibilityState,
      width: innerWidth,
      height: innerHeight,
      dpr: devicePixelRatio,
    }));
    const domHash = createHash('sha256').update(html).digest('hex');
    const frames = await h.page.evaluate(async () => {
      const box = document.querySelector('.conversation'),
        out = [];
      for (let repetition = -1; repetition < 20; repetition++) {
        box.scrollTop = 0;
        await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
        const gaps = [];
        let previous = performance.now();
        for (let i = 0; i < 36; i++) {
          box.scrollTop += 240;
          await new Promise((r) =>
            requestAnimationFrame((now) => {
              gaps.push(now - previous);
              previous = now;
              r();
            }),
          );
        }
        if (repetition >= 0) out.push(gaps);
      }
      return out;
    });
    const sorted = frames.flat().sort((a, b) => a - b);
    const row = {
      arm,
      windowState,
      domHash,
      samples: frames.length,
      frames: sorted.length,
      medianMs: sorted[Math.floor(sorted.length / 2)],
      p95Ms: sorted[Math.ceil(sorted.length * 0.95) - 1],
      maxMs: sorted.at(-1),
    };
    results.push(row);
    console.log(JSON.stringify(row));
  } finally {
    await h.close();
  }
}
await writeFile(
  root + 'scroll-investigation.json',
  JSON.stringify(
    {
      at: new Date().toISOString(),
      scope:
        '5000 mixed, fixed full DOM, no typing/streaming between scroll trials; A/B/A/B, warmup plus20 trials per launch',
      results,
    },
    null,
    2,
  ),
);
