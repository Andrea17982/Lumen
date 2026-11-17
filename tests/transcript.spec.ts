// Offline transcript regressions modelled on T3 Code's MessagesTimeline.browser.test.tsx
// scenarios (see src-tauri/resources/legal/t3code/ADAPTATIONS.md). Everything runs in the
// browser preview with fixture data; no provider, daemon or real app data is touched.
import { expect, test, type Page } from '@playwright/test';

const COUNT = 600;
const para = 'Variable height paragraph text that wraps across several lines. ';
function messages() {
  return Array.from({ length: COUNT }, (_, i) => {
    const n = String(i).padStart(4, '0');
    const user = i % 2 === 0;
    const shape = i % 5;
    const body =
      shape === 0
        ? 'Short.'
        : shape === 1
          ? para.repeat(12)
          : shape === 2
            ? '```ts\n' +
              Array.from({ length: 30 }, (_, l) => `const v${l} = ${l};`).join('\n') +
              '\n```'
            : shape === 3
              ? '- one\n- two\n- three\n\n' + para.repeat(3)
              : para;
    return {
      id: `m${n}`,
      role: user ? 'user' : 'assistant',
      text: `Message ${n}. ${body}`,
      activity:
        !user && i % 10 === 1
          ? [
              { kind: 'tool', text: `ran tool for ${n}` },
              { kind: 'command', text: `npm test # ${n}` },
            ]
          : [],
    };
  });
}

async function open(page: Page, extra: Record<string, unknown> = {}, list = messages()) {
  await page.addInitScript(
    ([list, extra]) =>
      localStorage.setItem(
        'lumen-preview-v1',
        JSON.stringify({
          accounts: [],
          sessions: [
            {
              id: 'long',
              title: 'Long transcript',
              status: 'completed',
              workspace: 'C:/Work',
              updatedAt: 2,
            },
            {
              id: 'other',
              title: 'Other chat',
              status: 'idle',
              workspace: 'C:/Work',
              updatedAt: 1,
            },
          ],
          messages: { long: list, other: [] },
          ...extra,
        }),
      ),
    [list, extra] as const,
  );
  await page.goto('/');
  await page.getByRole('button', { name: /^(?!Actions for).*Long transcript/ }).click();
  await expect(page.locator('article.message')).toHaveCount(list.length);
}

test('variable-height rows render complete and in order', async ({ page }) => {
  await open(page);
  const firsts = await page
    .locator('article.message')
    .evaluateAll((rows) => rows.map((r) => r.textContent?.match(/Message (\d{4})/)?.[1]));
  expect(firsts).toEqual(Array.from({ length: COUNT }, (_, i) => String(i).padStart(4, '0')));
  await page.locator('.conversation').evaluate((el) => (el.scrollTop = el.scrollHeight));
  await page.evaluate(
    () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))),
  );
  const heights = await page
    .locator('article.message')
    .evaluateAll((rows) => rows.slice(-5).map((r) => r.getBoundingClientRect().height));
  expect(new Set(heights.map(Math.round)).size).toBeGreaterThan(2);
  // Rows skipped by content-visibility stay in the accessibility tree.
  await expect(page.getByRole('article')).toHaveCount(COUNT);
});

test('long streaming appends in place and keeps earlier rows stable', async ({ page }) => {
  await open(page);
  const row = page.locator('article.message').nth(3);
  await expect(row.locator('.prose ul')).toBeVisible();
  const before = await row.innerText();
  await row.evaluate((el) => ((el as HTMLElement & { probe?: boolean }).probe = true));
  await page.evaluate(async () => {
    const send = (kind: string, text: string) =>
      window.dispatchEvent(
        new CustomEvent('lumen-preview-run', {
          detail: { sessionId: 'long', messageId: 'live', kind, text },
        }),
      );
    for (let i = 0; i < 400; i++) {
      send('delta', `token${i} `);
      if (i % 20 === 0) await new Promise((r) => requestAnimationFrame(r));
    }
  });
  const live = page.locator('article.message').last();
  await expect(live).toContainText('token0 token1');
  await expect(live).toContainText('token399');
  await expect(page.locator('article.message')).toHaveCount(COUNT + 1);
  // Same DOM node (not remounted) with the same content.
  expect(await row.evaluate((el) => (el as HTMLElement & { probe?: boolean }).probe)).toBe(true);
  expect(await row.innerText()).toBe(before);
});

test('activity expands and collapses by keyboard, and bounded output loads on demand', async ({
  page,
}) => {
  const list = messages().slice(0, 20);
  list.push({
    id: 'big',
    role: 'assistant',
    text: 'Large output follows.',
    activity: [
      {
        kind: 'tool',
        text: 'head of output\n… [truncated: showing 16384 of 40000 bytes]',
        detail: { digest: 'abc123', totalBytes: 40000, totalLines: 900, shownBytes: 16384 },
      } as never,
    ],
  });
  await open(page, { details: { abc123: 'head of output\nFULL TAIL LINE' } }, list);
  const big = page.locator('article.message').last();
  const summary = big.locator('details.activity > summary');
  await summary.focus();
  await page.keyboard.press('Enter');
  await expect(big.locator('details.activity')).toHaveAttribute('open', '');
  const tool = big.locator('.activity-content > details > summary');
  await page.keyboard.press('Tab');
  await expect(tool).toBeFocused();
  await page.keyboard.press('Space');
  await expect(big.getByText('40 KB')).toBeVisible();
  await expect(big.getByText('900 lines')).toBeVisible();
  await big.getByRole('button', { name: 'Show full output' }).click();
  await expect(big.getByText('FULL TAIL LINE')).toBeVisible();
  await expect(big.getByRole('button', { name: 'Show full output' })).toHaveCount(0);
  await summary.focus();
  await page.keyboard.press('Enter');
  await expect(big.locator('details.activity')).not.toHaveAttribute('open', '');
});

test('keyboard reaches controls in far-away rows and scrolls them into view', async ({ page }) => {
  await open(page);
  const early = page.locator('article.message').nth(1).locator('details.activity > summary');
  await early.focus();
  await expect(early).toBeFocused();
  await expect(early).toBeInViewport();
  await page.keyboard.press('Enter');
  await expect(early.locator('..')).toHaveAttribute('open', '');
  await page.keyboard.press('Tab');
  await expect(
    page.locator('article.message').nth(1).locator('.activity-content summary').first(),
  ).toBeFocused();
});

test('search finds conversations and text in rows far above the viewport', async ({ page }) => {
  await open(page);
  const target = page.locator('article.message').filter({ hasText: 'Message 0007.' });
  await expect(target).toHaveCount(1);
  await target.scrollIntoViewIfNeeded();
  await expect(target).toBeInViewport();
  // Native find-in-page searches content-visibility:auto content; assert it is not hidden.
  expect(await target.evaluate((el) => el.checkVisibility({ contentVisibilityAuto: false }))).toBe(
    true,
  );
  const search = page.getByRole('textbox', { name: 'Search conversations' });
  await search.fill('other');
  await expect(page.getByRole('button', { name: /^(?!Actions for).*Other chat/ })).toBeVisible();
  await expect(page.getByRole('button', { name: /^(?!Actions for).*Long transcript/ })).toHaveCount(
    0,
  );
});
