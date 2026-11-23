import assert from 'node:assert/strict';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { createHash } from 'node:crypto';
import os from 'node:os';
import { performanceHarness, privateProcesses } from './performance-harness.mjs';
import { seedPerformance } from './performance-fixtures.mjs';

const arm = process.argv[2] || 'baseline';
assert(['baseline', 'candidate', 'confirmation'].includes(arm));
const executable = path.resolve(
  path.join(
    process.env.LUMEN_PERF_ROOT || '.impeccable/performance',
    arm === 'candidate' ? 'candidate' : 'baseline',
    'lumen.exe',
  ),
);
const repetitions = Number(process.env.LUMEN_PERF_REPETITIONS || 20);
const h = await performanceHarness(executable);
const report = {
  arm,
  at: new Date().toISOString(),
  repetitions,
  build: 'Windows x64 release, embedded production frontend, isolated identifier',
  sha256: createHash('sha256')
    .update(await readFile(executable))
    .digest('hex'),
  startupMs: h.startupMs,
  environment: {
    os: os.release(),
    cpu: os.cpus()[0].model,
    logicalCpus: os.cpus().length,
    webview: await h.cdp.send('Browser.getVersion'),
  },
  scope:
    'Local synthetic UI; no accounts, no model calls; warm OS/profile, not cold OS. Timing includes IPC/UI, not provider inference.',
  samples: [],
};
const output = path.join(h.artifacts, `${arm}-results.json`);
const flush = () => writeFile(output, JSON.stringify(report, null, 2));
const settle = () =>
  h.page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
const metrics = async () =>
  Object.fromEntries(
    (await h.renderer.send('Performance.getMetrics')).metrics.map((m) => [m.name, m.value]),
  );
async function open(session) {
  const row = h.page.locator('.session-row').filter({ hasText: session.title });
  await row.evaluate((e) => e.click());
  await h.page.waitForFunction(
    ({ size }) =>
      document.querySelectorAll('.message').length === size &&
      document
        .querySelector('.message:last-of-type')
        ?.textContent.includes(`Fixture message ${size - 1}:`),
    { size: session.size },
  );
  await settle();
}
async function emptyChat() {
  const logo = h.page.locator('.sidebar-toggle');
  if ((await logo.getAttribute('aria-expanded')) === 'false') await logo.click();
  await h.page.getByRole('button', { name: 'New conversation', exact: true }).click();
  await logo.click();
  await settle();
}
try {
  const manifest = path.join(h.artifacts, 'sessions.json');
  let sessions;
  try {
    sessions = JSON.parse(await readFile(manifest, 'utf8'));
    assert(sessions.every((s) => s.workspace === h.workspace));
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
    sessions = await seedPerformance(h);
    await writeFile(manifest, JSON.stringify(sessions));
  }
  await h.page.reload();
  await h.page.locator('.session-row').first().waitFor();
  report.windowState = await h.page.evaluate(() => ({
    focused: document.hasFocus(),
    visibility: document.visibilityState,
    width: innerWidth,
    height: innerHeight,
    dpr: devicePixelRatio,
    reducedMotion: matchMedia('(prefers-reduced-motion: reduce)').matches,
  }));
  await open(sessions[0]);
  await emptyChat();
  await settle(); // module warmup, excluded
  await h.page.evaluate(() => {
    window.__perf = { frames: [], longTasks: [], keys: [], watch: false };
    let last = performance.now();
    function frame(now) {
      if (window.__perf.watch) window.__perf.frames.push(now - last);
      last = now;
      requestAnimationFrame(frame);
    }
    requestAnimationFrame(frame);
    new PerformanceObserver((list) => {
      if (window.__perf.watch)
        window.__perf.longTasks.push(...list.getEntries().map((e) => e.duration));
    }).observe({ type: 'longtask', buffered: false });
    document.addEventListener(
      'keydown',
      (e) => {
        if (e.target instanceof HTMLTextAreaElement && window.__perf.watch) {
          const at = performance.now();
          requestAnimationFrame(() =>
            setTimeout(() => window.__perf.keys.push(performance.now() - at), 0),
          );
        }
      },
      true,
    );
  });
  for (let repetition = 0; repetition < repetitions; repetition++) {
    for (const s of repetition % 2 ? [...sessions].reverse() : sessions) {
      if (
        process.env.LUMEN_PERF_WORKLOAD &&
        `${s.size}-${s.kind}` !== process.env.LUMEN_PERF_WORKLOAD
      )
        continue;
      await emptyChat();
      await settle();
      await h.page.evaluate(() =>
        Object.assign(window.__perf, { frames: [], longTasks: [], keys: [], watch: true }),
      );
      const start = performance.now();
      await open(s);
      const openMs = performance.now() - start;
      const input = h.page.locator('textarea').first();
      await input.fill('');
      await settle();
      const before = await metrics();
      const typingStart = performance.now();
      await input.pressSequentially('calibrazione');
      await settle();
      const typingMs = performance.now() - typingStart;
      const after = await metrics();
      assert.equal(await input.inputValue(), 'calibrazione');
      const streamBefore = await metrics();
      const streamStart = performance.now();
      await h.page.evaluate(
        async ({ id, lastId }) => {
          const emit = (kind, text) =>
            window.__TAURI_INTERNALS__.invoke('plugin:event|emit', {
              event: 'lumen:run',
              payload: { sessionId: id, messageId: lastId, kind, text },
            });
          await emit('status', 'running');
          for (let i = 0; i < 8; i++) {
            await emit('delta', ` measured-stream-${i}`);
            await new Promise((r) => setTimeout(r, 24));
          }
          await emit('status', 'completed');
        },
        { id: s.id, lastId: s.lastId },
      );
      await settle();
      const streamAfter = await metrics();
      const streamMs = performance.now() - streamStart;
      const scroll = await h.page.evaluate(async () => {
        const box = document.querySelector('.conversation');
        const frames = [];
        let last = performance.now();
        box.scrollTop = 0;
        await new Promise((r) => requestAnimationFrame(r));
        for (let i = 0; i < 18; i++) {
          box.scrollTop += 240;
          await new Promise((r) =>
            requestAnimationFrame((now) => {
              frames.push(now - last);
              last = now;
              r();
            }),
          );
        }
        for (const fraction of [0.5, 0, 1]) {
          box.scrollTop = fraction * box.scrollHeight;
          await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
        }
        return {
          frames,
          scrollHeight: box.scrollHeight,
          atBottom: box.scrollHeight - box.clientHeight - box.scrollTop < 4,
        };
      });
      const observed = await h.page.evaluate(() => {
        window.__perf.watch = false;
        return {
          frames: window.__perf.frames,
          longTasks: window.__perf.longTasks,
          keys: window.__perf.keys,
          articles: document.querySelectorAll('.message').length,
          domNodes: document.querySelectorAll('*').length,
        };
      });
      const sample = {
        size: s.size,
        kind: s.kind,
        repetition,
        openMs,
        typingMs,
        typingTaskMs: (after.TaskDuration - before.TaskDuration) * 1000,
        typingScriptMs: (after.ScriptDuration - before.ScriptDuration) * 1000,
        typingLayoutMs: (after.LayoutDuration - before.LayoutDuration) * 1000,
        streamMs,
        streamTaskMs: (streamAfter.TaskDuration - streamBefore.TaskDuration) * 1000,
        heapBytes: after.JSHeapUsedSize,
        scroll,
        ...observed,
      };
      if (
        process.env.LUMEN_PERF_SKIP_PRIVATE !== 'true' &&
        (repetition === 0 || repetition === repetitions - 1)
      )
        sample.processes = privateProcesses(h.pid);
      report.samples.push(sample);
      await flush();
      process.stdout.write(
        JSON.stringify({
          arm,
          case: `${s.size}-${s.kind}`,
          repetition,
          openMs: Math.round(openMs),
          typingTaskMs: sample.typingTaskMs,
        }) + '\n',
      );
    }
  }
  if (process.env.LUMEN_PERF_NO_CAPTURES !== 'true') {
    await open(sessions.find((s) => s.size === 1000 && s.kind === 'mixed'));
    await h.page.screenshot({ path: path.join(h.artifacts, `${arm}-light.png`) });
    await h.page.evaluate(() => (document.documentElement.dataset.theme = 'dark'));
    await h.page.screenshot({ path: path.join(h.artifacts, `${arm}-dark.png`) });
  }
  report.completedAt = new Date().toISOString();
  await flush();
} finally {
  await h.close();
}
