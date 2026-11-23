// Verify the ordinary (non-experimental) release against the frozen experiment's content gates.
import { readFile, writeFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { performanceHarness } from './performance-harness.mjs';

process.env.LUMEN_PERF_ROOT ||= '.impeccable/initial-open';
const root = path.resolve(process.env.LUMEN_PERF_ROOT);
const executable = path.join(root, 'candidate/lumen.exe');
const hash = (value) => createHash('sha256').update(value).digest('hex');
const experiment = JSON.parse(await readFile(path.join(root, 'experiment-results.json'), 'utf8'));
const reference = experiment.gates.find((g) => g.arm === 'plain');
const assets = await readdir('dist/assets');
assert(!assets.some((a) => a.includes('VirtualWindow')));
for (const asset of assets.filter((a) => a.endsWith('.js'))) {
  const text = await readFile(path.join('dist/assets', asset), 'utf8');
  assert(!/virtual-window-prototype|transcriptExperiment/.test(text), asset);
}
const h = await performanceHarness(executable);
const sessions = JSON.parse(await readFile(path.join(root, 'sessions.json'), 'utf8'));
async function open(size) {
  const session = sessions.find((s) => s.size === size && s.kind === 'mixed');
  await h.page.locator('.session-row').filter({ hasText: session.title }).click();
  await h.page.waitForFunction(
    (n) =>
      document
        .querySelector('.message.is-latest .prose')
        ?.textContent.includes(`PERF_SEARCH_${n - 1}`),
    size,
  );
  assert.equal(await h.page.locator('.message').count(), size);
  assert.equal(await h.page.locator('[data-virtual-window-prototype]').count(), 0);
}
try {
  const url = new URL(h.page.url());
  url.searchParams.set('transcriptExperiment', 'virtual');
  await h.page.goto(url.href);
  await open(5000);
  const htmlHash = hash(await h.page.locator('.message-list').evaluate((e) => e.innerHTML));
  assert.equal(htmlHash, reference.htmlHash);
  await open(1000);
  const selected = await h.page.evaluate(() => {
    const r = document.createRange();
    r.selectNodeContents(document.querySelector('.message-list'));
    const s = getSelection();
    s.removeAllRanges();
    s.addRange(r);
    return s.toString();
  });
  assert.equal(hash(selected), reference.selectionHash);
  const found = await h.page.evaluate(() => {
    getSelection().removeAllRanges();
    return window.find('PERF_SEARCH_0');
  });
  assert(found);
  const dom = await h.renderer.send('DOM.getDocument', { depth: 0 });
  const semantic = [];
  for (const selector of ['.message .prose p', '.message.is-latest .prose p']) {
    const target = await h.renderer.send('DOM.querySelector', {
      nodeId: dom.root.nodeId,
      selector,
    });
    assert(target.nodeId);
    const ax = await h.renderer.send('Accessibility.queryAXTree', { nodeId: target.nodeId });
    semantic.push(...ax.nodes.map((n) => [n.role?.value || '', n.name?.value || '', !!n.ignored]));
  }
  assert.equal(hash(JSON.stringify(semantic)), reference.axHash);
  await h.page.screenshot({ path: path.join(root, 'stock-light.png') });
  await h.page.evaluate(() => (document.documentElement.dataset.theme = 'dark'));
  await h.page.screenshot({ path: path.join(root, 'stock-dark.png') });
  const result = {
    at: new Date().toISOString(),
    sha256: hash(await readFile(executable)),
    noPrototypeAssets: true,
    queryCannotEnablePrototype: true,
    full5000HtmlMatches: true,
    selection1000Matches: true,
    nativeFind1000: true,
    targetedAx1000Matches: true,
    axScope: reference.axScope,
  };
  await writeFile(path.join(root, 'stock-results.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result));
} finally {
  await h.close();
}
