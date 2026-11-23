// Real Windows WebView2/IPC, offline CLI catalogs; no real account or inference.
import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { nativeHarness } from './native-harness.mjs';
const h = await nativeHarness('menu-layout');
const output = path.resolve('.codex/verification/menu-layout');
await mkdir(output, { recursive: true });
const rows = [];
try {
  for (const provider of [
    'claude',
    'gpt',
    'antigravity',
    'ollama',
    'opencode',
    'cursor',
    'openaiapi',
    'anthropicapi',
    'googleapi',
  ])
    await h.invoke('add_account', { provider, name: provider });
  await h.invoke('add_workspace', { name: 'Layout project', folder: h.workspace });
  const chat = await h.invoke('create_session', { workspace: h.workspace });
  await h.invoke('update_session', { id: chat.id, title: 'Menu layout probe', pinned: false });
  await h.page.reload();
  await h.page.locator('.session-row').filter({ hasText: 'Menu layout probe' }).click();
  await h.page
    .getByRole('group', { name: 'AI provider' })
    .getByRole('button', { name: 'Claude', exact: true })
    .click();
  const advanced = h.page.getByRole('button', { name: 'Advanced provider options' });
  if ((await advanced.getAttribute('aria-expanded')) === 'false') await advanced.click();
  await h.page.getByRole('combobox', { name: 'Model', exact: true }).waitFor();
  await h.page.waitForFunction(() => !document.querySelector('#model-picker').disabled);
  const settle = () =>
    h.page.evaluate(() => {
      document
        .getAnimations()
        .filter((a) => Number.isFinite(a.effect.getComputedTiming().endTime))
        .forEach((a) => a.finish());
    });
  for (const width of [480, 760, 850, 1276, 1920])
    for (const theme of ['light', 'dark']) {
      await h.page.setViewportSize({ width, height: 900 });
      await h.page.evaluate((theme) => (document.documentElement.dataset.theme = theme), theme);
      await settle();
      if (
        width <= 800 &&
        (await h.page.locator('.sidebar-toggle').getAttribute('aria-expanded')) === 'true'
      ) {
        await h.page.locator('.sidebar-toggle').click();
        await settle();
      }
      const picker = h.page.getByRole('group', { name: 'AI provider' });
      const buttons = await picker.getByRole('button').all();
      const boxes = await Promise.all(buttons.map((b) => b.boundingBox()));
      assert(
        boxes.every((b) => b && Math.abs(b.y - boxes[0].y) < 2),
        'Provider wrapped',
      );
      assert(
        await picker.evaluate(
          (e) => e.scrollWidth <= e.clientWidth || getComputedStyle(e).overflowX === 'auto',
        ),
        'Provider overflow must be scrollable',
      );
      const fields = await Promise.all(
        ['Account', 'Model', 'Reasoning effort'].map((name) =>
          h.page.getByRole('combobox', { name, exact: true }).boundingBox(),
        ),
      );
      assert(
        fields.every((b) => Math.abs(b.height - fields[0].height) < 1),
        'Unequal control heights',
      );
      for (const name of ['Account', 'Model', 'Reasoning effort', 'Project']) {
        const trigger = h.page.getByRole('combobox', { name, exact: true });
        await trigger.click();
        await settle();
        const a = await trigger.boundingBox(),
          b = await h.page.getByRole('listbox').boundingBox();
        assert(
          Math.min(Math.abs(b.y + b.height - a.y), Math.abs(b.y - a.y - a.height)) <= 8,
          `${name} detached`,
        );
        assert(
          b.x >= 0 && b.x + b.width <= width + 1 && b.y >= 0 && b.y + b.height <= 901,
          `${name} outside viewport`,
        );
        await h.page.screenshot({
          path: path.join(output, `${width}-${theme}-${name.replaceAll(' ', '-')}.png`),
        });
        await trigger.press('Escape');
      }
      const toggle = h.page.locator('.sidebar-toggle');
      if ((await toggle.getAttribute('aria-expanded')) === 'false') await toggle.click();
      for (const [name, dialog] of [
        ['Usage', 'Provider usage'],
        ['Context', 'Context window'],
      ]) {
        const trigger =
          name === 'Usage'
            ? h.page.getByRole('button', { name: 'Usage', exact: true })
            : h.page.locator('.context-meter');
        await trigger.click();
        await settle();
        const a = await trigger.boundingBox(),
          b = await h.page.getByRole('dialog', { name: dialog }).boundingBox();
        assert(Math.abs(b.y + b.height - a.y - a.height) < 8, `${name} detached vertically`);
        assert(b.y >= 0 && b.y + b.height <= 901, `${name} outside viewport`);
        await h.page.screenshot({ path: path.join(output, `${width}-${theme}-${name}.png`) });
        await h.page.locator('.usage-scrim').click({ position: { x: 1, y: 1 } });
      }
      rows.push({ width, theme, passed: true });
    }
  await writeFile(path.join(output, 'results.json'), JSON.stringify(rows, null, 2));
  console.log(
    'Passed: anchored four selectors, Usage/Context, equal field heights and single provider row at five sizes in both themes.',
  );
} finally {
  await h.close();
}
