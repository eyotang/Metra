import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './tests/ui', fullyParallel: true, forbidOnly: !!process.env.CI,
  outputDir: './test-results/ui',
  retries: process.env.CI ? 1 : 0, reporter: 'list',
  use: { baseURL: 'http://127.0.0.1:1420', viewport: { width: 360, height: 560 }, trace: 'retain-on-failure', timezoneId: 'Asia/Shanghai' },
  projects: [
    { name: 'chromium', use: { browserName: 'chromium', ...(process.env.PLAYWRIGHT_CHROME ? { channel: 'chrome' } : {}) } },
    { name: 'webkit', use: { browserName: 'webkit' } },
  ],
  webServer: { command: 'npm run frontend:build && vite preview --host 127.0.0.1 --port 1420 --strictPort', url: 'http://127.0.0.1:1420', reuseExistingServer: false },
});
