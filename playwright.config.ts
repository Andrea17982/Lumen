import { defineConfig } from '@playwright/test';
// LUMEN_E2E_PORT lets parallel checkouts run e2e without reusing another checkout's server.
const port = Number(process.env.LUMEN_E2E_PORT || 1420);
export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  use: { baseURL: `http://127.0.0.1:${port}`, browserName: 'chromium' },
  webServer: {
    command: `npm run dev -- --port ${port} --strictPort`,
    url: `http://127.0.0.1:${port}`,
    reuseExistingServer: !process.env.CI,
  },
});
