// Native release correctness gates for the full transcript. Synthetic data, no inference.
import { expect } from '@playwright/test';
import { DatabaseSync } from 'node:sqlite';
import { mkdir, readFile, writeFile, unlink } from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { performanceHarness } from './performance-harness.mjs';

const executable = path.resolve(
  process.env.LUMEN_PERF_EXE ||
    path.join(process.env.LUMEN_PERF_ROOT || '.impeccable/performance', 'candidate/lumen.exe'),
);
const h = await performanceHarness(executable);
const checks = [];
const created = [];
try {
  const fixture = JSON.parse(
    await readFile(path.join(h.artifacts, 'fixture-1000-mixed.json'), 'utf8'),
  );
  const image = await h.page.evaluate(() => {
    const c = document.createElement('canvas');
    c.width = 256;
    c.height = 160;
    const x = c.getContext('2d');
    x.fillStyle = '#3f6896';
    x.fillRect(0, 0, 256, 160);
    x.fillStyle = '#eef3f8';
    x.fillRect(32, 32, 192, 96);
    return c.toDataURL('image/png').split(',')[1];
  });
  await writeFile(path.join(h.workspace, 'fixture.png'), Buffer.from(image, 'base64'));
  await writeFile(path.join(h.workspace, 'counter.ts'), 'export const limit = 3;\n');
  const s = await h.invoke('create_session', { workspace: h.workspace });
  created.push(s.id);
  await h.invoke('update_session', { id: s.id, title: 'Long chat acceptance', pinned: false });
  fixture.forEach((m, i) => {
    m.id = `${s.id}-${i}`;
    if (i === 1) m.activity.push({ kind: 'thinking', text: 'A retained reasoning disclosure.' });
    if (i === 3) m.text += '\n\n![Fixture image](fixture.png)';
  });
  const db = new DatabaseSync(path.join(h.data, 'lumen.db'));
  const put = db.prepare(
    'INSERT INTO messages(id,session_id,role,text,activity,created_at) VALUES(?,?,?,?,?,?)',
  );
  db.exec('BEGIN');
  fixture.forEach((m, i) =>
    put.run(m.id, s.id, m.role, m.text, JSON.stringify(m.activity), Date.now() + i),
  );
  db.exec('COMMIT');
  db.close();
  await h.page.reload();
  await h.page.locator('.session-row').filter({ hasText: 'Long chat acceptance' }).click();
  await expect(h.page.locator('.message')).toHaveCount(1000);
  await h.page.waitForFunction(
    () => document.querySelectorAll('.message .prose pre').length === 500,
  );
  assert(await h.page.evaluate(() => window.find('PERF_SEARCH_0')));
  assert.equal(await h.page.evaluate(() => getSelection().toString()), 'PERF_SEARCH_0');
  checks.push('Browser text find reaches the first historical message through content-visibility');
  const selection = await h.page.evaluate(() => {
    const range = document.createRange();
    range.selectNodeContents(document.querySelector('.message-list'));
    const s = getSelection();
    s.removeAllRanges();
    s.addRange(range);
    return s.toString();
  });
  assert(
    selection.includes('PERF_SEARCH_0') &&
      selection.includes('PERF_SEARCH_999') &&
      selection.includes('export const limit = 3'),
  );
  await h.page.evaluate(() => getSelection().removeAllRanges());
  checks.push('Cross-history selection retains all 1000 messages and formatted code');
  const reason = h.page.locator('.message').nth(1).locator('.reasoning');
  await reason.locator('summary').click();
  await expect(reason).toHaveAttribute('open', '');
  await h.page.evaluate(() => {
    window.__historicalNode = document.querySelectorAll('.message')[1];
    navigator.clipboard.writeText = async (text) => {
      window.__copied = text;
    };
  });
  await h.page
    .locator('.message')
    .nth(1)
    .getByRole('button', { name: 'Copy response', exact: true })
    .click();
  assert((await h.page.evaluate(() => window.__copied)).includes('PERF_SEARCH_1'));
  await h.page.locator('textarea').first().fill('A preserved draft after disclosure and copy');
  assert(
    await h.page.evaluate(
      () => window.__historicalNode === document.querySelectorAll('.message')[1],
    ),
  );
  await expect(reason).toHaveAttribute('open', '');
  checks.push(
    'Copy returns the real response; typing preserves historical DOM and open disclosure',
  );
  const link = h.page
    .locator('.message')
    .nth(1)
    .getByRole('button', { name: 'Local source', exact: true });
  await link.focus();
  await expect(link).toBeFocused();
  await expect(link).toHaveAttribute('title', 'counter.ts:1');
  const img = h.page.getByRole('button', { name: 'Enlarge image: Fixture image', exact: true });
  await img.click();
  await expect(h.page.getByRole('dialog', { name: 'Fixture image', exact: true })).toBeVisible();
  await h.page.keyboard.press('Escape');
  await expect(img).toBeFocused();
  checks.push(
    'Historical link stays keyboard reachable; local image loads, enlarges and restores focus',
  );
  const tail = h.page.locator('.message').last();
  await h.invoke('plugin:event|emit', {
    event: 'lumen:run',
    payload: {
      sessionId: s.id,
      messageId: fixture.at(-1).id,
      kind: 'delta',
      text: ' LIVE_ACCEPTANCE_TAIL',
    },
  });
  await expect(tail).toContainText('LIVE_ACCEPTANCE_TAIL');
  await expect(reason).toHaveAttribute('open', '');
  assert(
    await h.page.evaluate(
      () => window.__historicalNode === document.querySelectorAll('.message')[1],
    ),
  );
  checks.push(
    'Streaming updates the latest body without replacing or collapsing historical messages',
  );
  await h.page.getByRole('button', { name: /Long conversation/ }).click();
  await expect(h.page.locator('.draft-sheet')).toBeVisible();
  await h.page.getByRole('button', { name: 'Close draft', exact: true }).click();
  checks.push('Compact handoff remains reachable without starting generation');
  // Exercise stable callbacks after a chat switch: the current session has four later messages.
  const otherWorkspace = path.join(h.artifacts, 'callback-workspace');
  await mkdir(otherWorkspace, { recursive: true });
  const other = await h.invoke('create_session', { workspace: otherWorkspace });
  created.push(other.id);
  await h.invoke('update_session', { id: other.id, title: 'Callback target chat', pinned: false });
  const db2 = new DatabaseSync(path.join(h.data, 'lumen.db'));
  const insert = db2.prepare(
    'INSERT INTO messages(id,session_id,role,text,activity,created_at) VALUES(?,?,?,?,?,?)',
  );
  for (let i = 0; i < 4; i++)
    insert.run(
      `${other.id}-${i}`,
      other.id,
      i % 2 ? 'assistant' : 'user',
      `Callback target ${i}`,
      '[]',
      Date.now() + i,
    );
  db2.close();
  await h.page.reload();
  await h.page.locator('.session-row').filter({ hasText: 'Long chat acceptance' }).click();
  await expect(h.page.locator('.message')).toHaveCount(1000);
  await h.page.locator('.session-row').filter({ hasText: 'Callback target chat' }).click();
  await expect(h.page.locator('.message')).toHaveCount(4);
  await h.page.locator('.message').first().getByRole('button', { name: 'Rewind to here' }).click();
  await expect(h.page.getByRole('dialog', { name: 'Rewind', exact: true })).toContainText(
    '4 messages will be removed',
  );
  await h.page
    .getByRole('dialog', { name: 'Rewind', exact: true })
    .getByRole('button', { name: 'Cancel' })
    .click();
  checks.push(
    'Memoized message actions use the latest chat after navigation, not captured old state',
  );
  const comparison = await h.invoke('create_session', { workspace: h.workspace });
  created.push(comparison.id);
  await h.invoke('update_session', {
    id: comparison.id,
    title: 'Comparison acceptance',
    pinned: false,
  });
  const db3 = new DatabaseSync(path.join(h.data, 'lumen.db'));
  const laneInsert = db3.prepare(
    'INSERT INTO messages(id,session_id,role,text,activity,created_at) VALUES(?,?,?,?,?,?)',
  );
  for (const [index, provider] of ['claude', 'codex'].entries())
    laneInsert.run(
      `${comparison.id}-${index}`,
      comparison.id,
      'assistant',
      `Preserved ${provider} comparison`,
      JSON.stringify([
        { kind: 'lane', text: provider },
        { kind: 'applied', text: 'Controlled existing result' },
      ]),
      Date.now() + index,
    );
  db3.close();
  await h.page.reload();
  await h.page.locator('.session-row').filter({ hasText: 'Comparison acceptance' }).click();
  await expect(h.page.locator('.lanes')).toHaveCount(1);
  await expect(h.page.locator('.lane-grid .message')).toHaveCount(2);
  await expect(h.page.locator('.lane-grid')).toContainText('Preserved claude comparison');
  await expect(h.page.locator('.lane-grid')).toContainText('Preserved codex comparison');
  checks.push('Both comparison messages retain adjacent lane grouping and distinct visible bodies');
  const errors = [];
  h.page.on('pageerror', (e) => errors.push(e.message));
  for (const width of [1240, 800, 720]) {
    await h.page.setViewportSize({ width, height: 820 });
    await h.page.locator('textarea').first().fill(`Viewport ${width}`);
    assert(await h.page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
  }
  assert.equal(errors.length, 0);
  await writeFile(
    path.join(h.artifacts, 'acceptance-results.json'),
    JSON.stringify(
      {
        at: new Date().toISOString(),
        sha256: createHash('sha256')
          .update(await readFile(executable))
          .digest('hex'),
        passed: true,
        checks,
        scope:
          'Native Windows release, synthetic fixtures. No screen-reader certification, external-editor launch, provider inference or universal accessibility claim.',
      },
      null,
      2,
    ),
  );
  console.log(JSON.stringify({ passed: true, checks }, null, 2));
} finally {
  for (const id of created) await h.invoke('delete_session', { id }).catch(() => {});
  for (const name of ['fixture.png', 'counter.ts'])
    await unlink(path.join(h.workspace, name)).catch((error) => {
      if (error.code !== 'ENOENT') throw error;
    });
  await h.close();
}
