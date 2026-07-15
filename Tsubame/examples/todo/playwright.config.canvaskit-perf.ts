import { existsSync } from 'node:fs';
import { defineConfig, devices } from '@playwright/test';

const PREINSTALLED_CHROMIUM = '/opt/pw-browsers/chromium';
const SYSTEM_CHROME = '/usr/bin/google-chrome';
const executablePath = existsSync(PREINSTALLED_CHROMIUM)
  ? PREINSTALLED_CHROMIUM
  : existsSync(SYSTEM_CHROME)
    ? SYSTEM_CHROME
    : undefined;
const PORT = Number(process.env.E2E_CANVASKIT_PERF_PORT ?? 5190);

export default defineConfig({
  testDir: './e2e',
  testMatch: 'canvaskit-performance.spec.ts',
  fullyParallel: false,
  workers: 1,
  retries: 0,
  timeout: 90_000,
  reporter: process.env.CI ? 'list' : [['list'], ['html', { open: 'never' }]],
  use: {
    ...devices['Desktop Chrome'],
    baseURL: `http://localhost:${PORT}`,
    launchOptions: {
      executablePath,
      args: ['--enable-webgl', '--ignore-gpu-blocklist'],
    },
    trace: 'retain-on-failure',
  },
  webServer: {
    command: `./node_modules/.bin/vite --port ${PORT} --strictPort`,
    url: `http://localhost:${PORT}`,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
