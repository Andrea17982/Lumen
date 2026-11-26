import { defineConfig } from '@playwright/test';

// UI tests run against the Vite build of the client in Microsoft Edge (Chromium, like WebView2),
// so no browser download is needed on Windows.
export default defineConfig({
  testDir: 'tests/ui',
  fullyParallel: true,
  reporter: [['list']],
  use: { baseURL: 'http://127.0.0.1:4173', channel: 'msedge', viewport: { width: 1240, height: 820 } },
  // The CI Windows runner needs far longer than the 5 s default; passing runs are not slowed,
  // an expect resolves as soon as its condition holds.
  expect: { timeout: 20_000, toHaveScreenshot: { maxDiffPixelRatio: 0.002 } },
  webServer: { command: 'npx vite build && npx vite preview --port 4173 --strictPort', env: { VITE_TEST_BRIDGE: '1' }, url: 'http://127.0.0.1:4173', reuseExistingServer: !process.env.CI, timeout: 120_000 },
});
