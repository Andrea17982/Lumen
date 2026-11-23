// Release A/B/C in one instrumented executable; no inference or private account data.
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { performanceHarness, privateProcesses } from './performance-harness.mjs';
import { seedPerformance } from './performance-fixtures.mjs';

process.env.LUMEN_PERF_ROOT ||= '.impeccable/initial-open';
const root = path.resolve(process.env.LUMEN_PERF_ROOT);
const executable = path.join(root, 'experiment/lumen.exe');
const h = await performanceHarness(executable);
const repetitions = Number(process.env.LUMEN_OPEN_REPETITIONS || 20);
const secondary = process.env.LUMEN_OPEN_SECONDARY === '1';
const result = {
  at: new Date().toISOString(),
  sha256: createHash('sha256')
    .update(await readFile(executable))
    .digest('hex'),
  repetitions,
  scope: secondary
    ? 'Secondary investigation: 100 mixed opening and 1000 mixed typing'
    : 'Fixed six-workload comparison',
  build: 'Isolated Windows x64 release experiment, one binary and fixture for all arms',
  environment: {
    webview: await h.cdp.send('Browser.getVersion'),
    window: await h.page.evaluate(() => ({
      width: innerWidth,
      height: innerHeight,
      dpr: devicePixelRatio,
    })),
  },
  samples: [],
  gates: [],
};
const flush = () =>
  writeFile(
    path.join(root, secondary ? 'secondary-results.json' : 'experiment-results.json'),
    JSON.stringify(result, null, 2),
  );
const settle = () =>
  h.page.evaluate(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
const metrics = async () =>
  Object.fromEntries(
    (await h.renderer.send('Performance.getMetrics')).metrics.map((m) => [m.name, m.value]),
  );
async function mode(name) {
  const url = new URL(h.page.url());
  url.searchParams.set('transcriptExperiment', name);
  await h.page.goto(url.href);
  await h.page.locator('.session-row').first().waitFor();
}
async function open(s) {
  await h.page
    .locator('.session-row')
    .filter({ hasText: s.title })
    .evaluate((e) => e.click());
  await h.page.waitForFunction(
    ({ n }) =>
      document
        .querySelector('.message.is-latest .prose')
        ?.textContent.includes(`PERF_SEARCH_${n - 1}`),
    { n: s.size },
  );
  await settle();
}
try {
  let sessions;
  const manifest = path.join(root, 'sessions.json');
  try {
    sessions = JSON.parse(await readFile(manifest, 'utf8'));
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
    sessions = await seedPerformance(h);
    await writeFile(manifest, JSON.stringify(sessions));
  }
  const arms = secondary ? ['baseline', 'plain'] : ['baseline', 'plain', 'virtual'];
  const cases = secondary ? sessions.filter((s) => s.kind === 'mixed' && s.size <= 1000) : sessions;
  for (let repetition = 0; repetition < repetitions; repetition++)
    for (const arm of repetition % 2 ? [...arms].reverse() : arms) {
      await mode(arm);
      await open(sessions[0]);
      await h.page.locator('.new-chat').first().click();
      await settle();
      for (const s of repetition % 2 ? [...cases].reverse() : cases) {
        await h.page.locator('.new-chat').first().click();
        await settle();
        await h.page.evaluate(() => {
          window.__openTasks = [];
          window.__openObserver = new PerformanceObserver((list) =>
            window.__openTasks.push(...list.getEntries().map((e) => e.duration)),
          );
          window.__openObserver.observe({ type: 'longtask', buffered: false });
        });
        const before = await metrics();
        const start = performance.now();
        await open(s);
        const openMs = performance.now() - start;
        const after = await metrics();
        const state = await h.page.evaluate(() => {
          window.__openObserver.disconnect();
          return {
            articles: document.querySelectorAll('.message').length,
            nodes: document.querySelectorAll('*').length,
            virtual: !!document.querySelector('[data-virtual-window-prototype]'),
            longTasks: window.__openTasks,
            focused: document.hasFocus(),
            visibility: document.visibilityState,
          };
        });
        if (arm !== 'virtual') assert.equal(state.articles, s.size);
        assert.equal(
          state.virtual,
          arm === 'virtual',
          'Experiment arm did not select its intended renderer',
        );
        const input = h.page.locator('textarea').first();
        await input.fill('');
        await settle();
        const typingBefore = await metrics();
        await input.pressSequentially('calibrazione');
        await settle();
        const typingAfter = await metrics();
        const row = {
          arm,
          repetition,
          size: s.size,
          kind: s.kind,
          openMs,
          taskMs: (after.TaskDuration - before.TaskDuration) * 1000,
          scriptMs: (after.ScriptDuration - before.ScriptDuration) * 1000,
          layoutMs: (after.LayoutDuration - before.LayoutDuration) * 1000,
          typingTaskMs: (typingAfter.TaskDuration - typingBefore.TaskDuration) * 1000,
          heapBytes: after.JSHeapUsedSize,
          ...state,
        };
        if (repetition === 0) {
          row.processes = privateProcesses(h.pid);
          row.ipcProbe = await h.page.evaluate(async (id) => {
            const t = performance.now();
            const m = await window.__TAURI_INTERNALS__.invoke('get_messages', { id });
            return { ms: performance.now() - t, count: m.length };
          }, s.id);
        }
        result.samples.push(row);
        await flush();
        console.log(
          JSON.stringify({
            arm,
            repetition,
            case: `${s.size}-${s.kind}`,
            openMs: Math.round(openMs),
            articles: state.articles,
          }),
        );
      }
    }
  if (!secondary)
    for (const arm of ['baseline', 'plain', 'virtual']) {
      result.gateStage = { arm, stage: 'opening-5000' };
      await flush();
      await mode(arm);
      const s = sessions.find((s) => s.size === 5000 && s.kind === 'mixed');
      await open(s);
      const html = await h.page.locator('.message-list').evaluate((e) => e.innerHTML);
      result.gateStage = { arm, stage: 'selection-1000' };
      await flush();
      const small = sessions.find((s) => s.size === 1000 && s.kind === 'mixed');
      await open(small);
      const selection = await h.page.evaluate(() => {
        const r = document.createRange();
        r.selectNodeContents(document.querySelector('.message-list'));
        const s = getSelection();
        s.removeAllRanges();
        s.addRange(r);
        return s.toString();
      });
      const found = await h.page.evaluate(() => {
        getSelection().removeAllRanges();
        return window.find('PERF_SEARCH_0');
      });
      result.gateStage = { arm, stage: 'targeted-ax-1000' };
      await flush();
      const dom = await h.renderer.send('DOM.getDocument', { depth: 0 });
      const semantic = [];
      for (const selector of ['.message .prose p', '.message.is-latest .prose p']) {
        const target = await h.renderer.send('DOM.querySelector', {
          nodeId: dom.root.nodeId,
          selector,
        });
        if (target.nodeId) {
          const ax = await h.renderer.send('Accessibility.queryAXTree', { nodeId: target.nodeId });
          semantic.push(
            ...ax.nodes.map((n) => [n.role?.value || '', n.name?.value || '', !!n.ignored]),
          );
        }
      }
      const gate = {
        arm,
        nativeSize: 1000,
        htmlHash: createHash('sha256').update(html).digest('hex'),
        axHash: createHash('sha256').update(JSON.stringify(semantic)).digest('hex'),
        axNodes: semantic.length,
        axScope:
          'Targeted first/last paragraph subtrees on 1000 mixed messages, not full-tree export or screen-reader certification',
        axFirst: semantic.some((n) => n[1].includes('PERF_SEARCH_0')),
        axLast: semantic.some((n) => n[1].includes('PERF_SEARCH_999')),
        findFirst: found,
        selectionFirst: selection.includes('PERF_SEARCH_0'),
        selectionLast: selection.includes('PERF_SEARCH_999'),
        selectionHash: createHash('sha256').update(selection).digest('hex'),
      };
      result.gates.push(gate);
      await flush();
      console.log(JSON.stringify(gate));
      await h.page.screenshot({ path: path.join(root, `${arm}-light.png`) });
      await h.page.evaluate(() => (document.documentElement.dataset.theme = 'dark'));
      await h.page.screenshot({ path: path.join(root, `${arm}-dark.png`) });
    }
  result.completedAt = new Date().toISOString();
  await flush();
} finally {
  await h.close();
}
