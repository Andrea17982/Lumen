// Offline long-transcript timing in the browser preview (fixture data only; no app data).
// Usage: build with `npx vite build --outDir <dir>`, serve with `npx vite preview --outDir <dir>
// --port <port>`, then `node tests/manual/transcript-performance.mjs http://127.0.0.1:<port> [label]`.
import { chromium } from '@playwright/test';

const url = process.argv[2] || 'http://127.0.0.1:1420';
const label = process.argv[3] || 'run';
const COUNT = 2000;
const para = 'Variable height paragraph text that wraps across several lines. ';
const messages = Array.from({ length: COUNT }, (_, i) => ({
  id: `m${i}`,
  role: i % 2 ? 'assistant' : 'user',
  text:
    `Message ${i}. ` +
    (i % 4 === 1 ? '```js\n' + 'const x = 1;\n'.repeat(20) + '```\n' : '') +
    para.repeat(1 + (i % 6)),
  activity: i % 10 === 1 ? [{ kind: 'tool', text: 'ran tool' }] : [],
}));
const stats = (xs) => {
  const s = [...xs].sort((a, b) => a - b);
  const q = (p) => +s[Math.min(s.length - 1, Math.floor(p * s.length))].toFixed(1);
  return { n: s.length, p50: q(0.5), p95: q(0.95), max: q(1) };
};
const browser = await chromium.launch();
const results = [];
for (let round = 0; round < 3; round++) {
  const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
  await page.addInitScript(
    (list) =>
      localStorage.setItem(
        'lumen-preview-v1',
        JSON.stringify({
          accounts: [],
          sessions: [
            { id: 'long', title: 'Long', status: 'completed', workspace: 'C:/W', updatedAt: 1 },
          ],
          messages: { long: list },
        }),
      ),
    messages,
  );
  await page.goto(url);
  const t0 = Date.now();
  await page.getByRole('button', { name: /^(?!Actions for).*Long/ }).click();
  await page.waitForFunction(
    (n) => document.querySelectorAll('article.message').length === n,
    COUNT,
  );
  const open = Date.now() - t0;
  const stream = await page.evaluate(async () => {
    const gaps = [];
    let last = performance.now();
    for (let i = 0; i < 240; i++) {
      window.dispatchEvent(
        new CustomEvent('lumen-preview-run', {
          detail: { sessionId: 'long', messageId: 'live', kind: 'delta', text: `token${i} ` },
        }),
      );
      await new Promise((r) =>
        requestAnimationFrame((now) => {
          gaps.push(now - last);
          last = now;
          r();
        }),
      );
    }
    return gaps.slice(5);
  });
  const scroll = await page.evaluate(async () => {
    const box = document.querySelector('.conversation');
    box.scrollTop = 0;
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
    const gaps = [];
    let last = performance.now();
    for (let i = 0; i < 120; i++) {
      box.scrollTop += 400;
      await new Promise((r) =>
        requestAnimationFrame((now) => {
          gaps.push(now - last);
          last = now;
          r();
        }),
      );
    }
    return gaps;
  });
  results.push({ open, stream: stats(stream), scroll: stats(scroll) });
  await page.close();
}
await browser.close();
console.log(JSON.stringify({ label, messages: COUNT, results }, null, 1));
