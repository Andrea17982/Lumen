// Final source changed after the three-arm series: confirm the normal binary against
// the frozen experiment baseline, retaining separate hashes and all 20 paired trials.
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { performanceHarness } from './performance-harness.mjs';

process.env.LUMEN_PERF_ROOT ||= '.impeccable/initial-open';
const root = path.resolve(process.env.LUMEN_PERF_ROOT);
const sessions = JSON.parse(await readFile(path.join(root, 'sessions.json'), 'utf8'));
const mixed = (size) => sessions.find((s) => s.size === size && s.kind === 'mixed');
const binaries = {
  baseline: path.join(root, 'experiment/lumen.exe'),
  candidate: path.join(root, 'candidate/lumen.exe'),
};
const result = { at: new Date().toISOString(), hashes: {}, samples: [] };
const probe = process.env.LUMEN_OPEN_CONFIRMATION_PROBE === '1';
const output = probe ? 'confirmation-probe-results.json' : 'confirmation-results.json';
for (const [arm, executable] of Object.entries(binaries))
  result.hashes[arm] = createHash('sha256')
    .update(await readFile(executable))
    .digest('hex');
for (let repetition = 0; repetition < (probe ? 2 : 20); repetition++)
  for (const arm of repetition % 2 ? ['candidate', 'baseline'] : ['baseline', 'candidate']) {
    const h = await performanceHarness(binaries[arm]);
    try {
      // Equal navigation after reduced-motion setup; normal builds ignore the selector.
      const url = new URL(h.page.url());
      url.searchParams.set('transcriptExperiment', 'baseline');
      await h.page.goto(url.href);
      const settle = () =>
        h.page.evaluate(
          () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))),
        );
      const metrics = async () =>
        Object.fromEntries(
          (await h.renderer.send('Performance.getMetrics')).metrics.map((m) => [m.name, m.value]),
        );
      async function open(s) {
        await h.page
          .locator('.session-row')
          .filter({ hasText: s.title })
          .evaluate((e) => e.click());
        await h.page.waitForFunction(
          (n) =>
            document
              .querySelector('.message.is-latest .prose')
              ?.textContent.includes(`PERF_SEARCH_${n - 1}`),
          s.size,
        );
        await settle();
      }
      await open(mixed(100)); // Equal per-launch warmup, excluded from timing.
      await h.page.locator('.new-chat').first().click();
      await settle();
      const before = await metrics();
      const start = performance.now();
      await open(mixed(5000));
      const openMs = performance.now() - start;
      const after = await metrics();
      assert.equal(await h.page.locator('.message').count(), 5000);
      assert.equal(await h.page.locator('[data-virtual-window-prototype]').count(), 0);
      const state = await h.page.evaluate(() => ({
        focused: document.hasFocus(),
        visibility: document.visibilityState,
      }));
      const typingBefore = await metrics();
      await h.page.locator('textarea').first().pressSequentially('calibrazione');
      await settle();
      const typingAfter = await metrics();
      result.samples.push({
        arm,
        repetition,
        openMs,
        taskMs: (after.TaskDuration - before.TaskDuration) * 1000,
        typingTaskMs: (typingAfter.TaskDuration - typingBefore.TaskDuration) * 1000,
        ...state,
      });
      await writeFile(path.join(root, output), JSON.stringify(result, null, 2));
      console.log(JSON.stringify({ arm, repetition, openMs: Math.round(openMs) }));
    } finally {
      await h.close();
    }
  }
result.completedAt = new Date().toISOString();
await writeFile(path.join(root, output), JSON.stringify(result, null, 2));
