import { defineConfig } from '@playwright/test';

// UI tests run against the Vite build of the client in Microsoft Edge (Chromium, like WebView2),
// so no browser download is needed on Windows.
export default defineConfig({
  testDir: 'tests/ui',
  fullyParallel: true,
  reporter: [['list']],
  use: { baseURL: 'http://127.0.0.1:4173', channel: 'msedge', viewport: { width: 1240, height: 820 } },
  expect: { toHaveScreenshot: { maxDiffPixelRatio: 0.002 } },
  webServer: { command: 'npx vite build && npx vite preview --port 4173 --strictPort', url: 'http://127.0.0.1:4173', reuseExistingServer: !process.env.CI, timeout: 120_000 },
});
