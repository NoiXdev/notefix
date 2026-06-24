import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  reporter: 'list',
  // One Vite dev server shared by all workers → occasional load-induced
  // timeouts on interaction-heavy specs. A single retry absorbs that.
  retries: 1,
  timeout: 30_000,
  use: { baseURL: 'http://localhost:1420', locale: 'de-DE', trace: 'on-first-retry' },
  webServer: {
    command: 'npm run dev',
    url: 'http://localhost:1420',
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
});
