// Diagnose the final comparison's unequal navigation/media initialization, without
// modifying application code or inferring a rendering regression from mixed scroll modes.
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import assert from 'node:assert/strict';
import { performanceHarness } from './performance-harness.mjs';
process.env.LUMEN_PERF_ROOT ||= '.impeccable/initial-open';
const root = path.resolve(process.env.LUMEN_PERF_ROOT);
const h = await performanceHarness(path.join(root, 'candidate/lumen.exe'));
const session = JSON.parse(await readFile(path.join(root, 'sessions.json'), 'utf8')).find(
  (s) => s.size === 5000 && s.kind === 'mixed',
);
const observations = [];
try {
  for (const stage of ['initial', 'reloaded']) {
    if (stage === 'reloaded') await h.page.reload();
    await h.page.evaluate(() => {
      window.__scrollBehaviors = [];
      const original = Element.prototype.scrollIntoView;
      Element.prototype.scrollIntoView = function (options) {
        window.__scrollBehaviors.push(options?.behavior);
        return original.call(this, options);
      };
    });
    await h.page.locator('.session-row').filter({ hasText: session.title }).click();
    await h.page.waitForFunction(() => document.querySelectorAll('.message').length === 5000);
    await h.page.evaluate(
      () => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))),
    );
    observations.push(
      await h.page.evaluate(
        (stage) => ({
          stage,
          mediaReduced: matchMedia('(prefers-reduced-motion: reduce)').matches,
          behaviors: window.__scrollBehaviors,
        }),
        stage,
      ),
    );
  }
  assert(observations.every((o) => o.mediaReduced));
  assert(observations[0].behaviors.includes('smooth'));
  assert(observations[1].behaviors.includes('instant'));
  await writeFile(
    path.join(root, 'motion-diagnostic-results.json'),
    JSON.stringify(
      {
        observations,
        conclusion:
          'Media emulation occurs after initial hook mount. A reload initializes the installed hook with reduced motion; both timing arms must navigate equally after emulation.',
      },
      null,
      2,
    ),
  );
  console.log(JSON.stringify(observations));
} finally {
  await h.close();
}
