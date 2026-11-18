import { expect, test } from '@playwright/test';
import { PANEL_CSP, PANEL_SANDBOX } from '../src/lib/plugins';

// A real browser check of the isolation a plugin panel gets: the same sandbox attribute and CSP
// header the desktop app serves, with a parent page that has a fake Tauri bridge to steal.
const hostile = `<!doctype html><script>
const out = {};
try { out.parentDom = !!parent.document.body; } catch { out.parentDom = false; }
try { out.tauri = typeof parent.__TAURI_INTERNALS__; } catch { out.tauri = 'blocked'; }
out.ownTauri = typeof window.__TAURI_INTERNALS__;
try { localStorage.setItem('x', '1'); out.storage = true; } catch { out.storage = false; }
out.origin = self.origin;
fetch('https://example.com/').then(() => { out.fetch = true; }, () => { out.fetch = false; })
  .finally(() => parent.postMessage({ lumen: 1, id: 1, method: 'report', params: out }, '*'));
</script>`;

test('plugin panels cannot reach Lumen, its IPC, storage or the network', async ({ page }) => {
  await page.route('http://lumen-plugin.localhost/**', (route) =>
    route.fulfill({
      status: 200,
      headers: { 'content-type': 'text/html', 'content-security-policy': PANEL_CSP },
      body: hostile,
    }),
  );
  await page.route('https://example.com/**', (route) =>
    route.fulfill({ status: 200, body: 'leak' }),
  );
  await page.route('http://host.test/', (route) =>
    route.fulfill({ status: 200, headers: { 'content-type': 'text/html' }, body: '<body></body>' }),
  );
  await page.goto('http://host.test/');
  const report = await page.evaluate(
    (sandbox) =>
      new Promise<Record<string, unknown>>((resolve) => {
        (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = { invoke() {} };
        window.addEventListener('message', (e) => resolve(e.data.params));
        const frame = document.createElement('iframe');
        frame.setAttribute('sandbox', sandbox);
        frame.src = 'http://lumen-plugin.localhost/demo/main';
        document.body.append(frame);
      }),
    PANEL_SANDBOX,
  );
  expect(report).toEqual({
    parentDom: false,
    tauri: 'blocked',
    ownTauri: 'undefined',
    storage: false,
    origin: 'null',
    fetch: false,
  });
});
