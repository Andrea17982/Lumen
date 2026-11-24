// The M0 budget as a regression guard: a 5,000-message session from the real daemon opens with
// the newest messages at once, mounts the rest without freezing the window, keeps every message
// in the DOM for native find, and keeps typing responsive. Limits are loose enough for CI runners;
// the measured values are printed so drift shows before a limit fails.
import { expect, test } from '@playwright/test';
import { seed, transcript } from '../support/transcript.mjs';
import { startDaemon } from './daemon-fixture';

test.skip(process.platform !== 'win32', 'the daemon fixtures run on Windows');

const COUNT = 5000;
// Measured on the owner's machine (24/11): 0.3 s, 3.8 s, 17–21 ms, 3–5 ms.
const budget = { firstMessagesMs: 1500, allMountedMs: 12_000, frameP95Ms: 50, typingP95Ms: 50 };

test(`a ${COUNT}-message conversation stays within the M0 budget`, async ({ page, baseURL }) => {
  test.setTimeout(180_000);
  const daemon = await startDaemon(new URL(baseURL!).origin);
  try {
    const chat = await daemon.client.call('create_session', { workspace: daemon.workspace });
    await daemon.stop();
    seed(daemon.data, chat.id, transcript(COUNT, 5));
    await daemon.restart();

    await page.goto(daemon.url());
    await expect(page.getByRole('status')).toHaveText('Connected');
    // Frame gaps from the moment the conversation opens.
    await page.evaluate(() => {
      const w = window as unknown as { gaps: number[] };
      w.gaps = [];
      let last = performance.now();
      const tick = (now: number) => {
        w.gaps.push(now - last);
        last = now;
        requestAnimationFrame(tick);
      };
      requestAnimationFrame(tick);
    });
    const start = Date.now();
    await page.getByRole('navigation').getByRole('button').first().click();
    await expect(page.getByText('ZEBRALAST marker in the last message.')).toBeVisible();
    const firstMessagesMs = Date.now() - start;

    // Type while the earlier pages load and mount. As in M0: from the key event to the next frame
    // (headless Edge reports no real presentation times, so Event Timing durations are not usable).
    await page.evaluate(() => {
      const w = window as unknown as { typing: number[] };
      w.typing = [];
      addEventListener('keydown', (e) => requestAnimationFrame((frame) => w.typing.push(frame - e.timeStamp)), true);
    });
    await page.getByRole('textbox', { name: 'Describe what you want to do' }).pressSequentially('typing while the transcript mounts', { delay: 30 });

    await expect(page.getByRole('log').getByRole('article')).toHaveCount(COUNT, { timeout: budget.allMountedMs });
    const allMountedMs = Date.now() - start;
    const p95 = (values: number[]) => (values.length ? [...values].sort((a, b) => a - b)[Math.floor(values.length * 0.95)] : 0);
    const frameP95Ms = p95(await page.evaluate(() => (window as unknown as { gaps: number[] }).gaps.slice(1)));
    const typingP95Ms = p95(await page.evaluate(() => (window as unknown as { typing: number[] }).typing));
    // Every message is in the DOM, so native find reaches the first one without scrolling.
    expect(await page.evaluate(() => document.body.textContent!.includes('ZEBRAFIRST marker in the first message.'))).toBe(true);
    console.log(JSON.stringify({ firstMessagesMs, allMountedMs, frameP95Ms: Math.round(frameP95Ms), typingP95Ms: Math.round(typingP95Ms) }));
    expect(firstMessagesMs).toBeLessThan(budget.firstMessagesMs);
    expect(frameP95Ms).toBeLessThan(budget.frameP95Ms);
    expect(typingP95Ms).toBeLessThan(budget.typingP95Ms);
  } finally {
    await daemon.close();
  }
});
