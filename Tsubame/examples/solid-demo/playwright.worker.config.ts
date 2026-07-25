import { existsSync } from 'node:fs';
import { defineConfig } from '@playwright/test';

const PREINSTALLED_CHROMIUM = '/opt/pw-browsers/chromium';
const SYSTEM_CHROME = '/usr/bin/google-chrome';
const executablePath = existsSync(PREINSTALLED_CHROMIUM)
  ? PREINSTALLED_CHROMIUM
  : existsSync(SYSTEM_CHROME)
    ? SYSTEM_CHROME
    : undefined;
const WORKER_WORKLOAD_PORT = 5182;
const WEBGPU_LAUNCH_ARGS = [
  '--enable-unsafe-webgpu',
  '--enable-features=Vulkan',
  '--enable-gpu',
  '--use-angle=vulkan',
] as const;

export default defineConfig({
  testDir: './e2e',
  testMatch: 'canvas-worker-touch-scroll.spec.ts',
  fullyParallel: false,
  workers: 1,
  reporter: 'list',
  use: {
    baseURL: `http://127.0.0.1:${WORKER_WORKLOAD_PORT}`,
    screenshot: 'only-on-failure',
    trace: 'retain-on-failure',
  },
  projects: [
    {
      name: 'mobile-chromium-webgpu',
      use: {
        browserName: 'chromium',
        launchOptions: {
          executablePath,
          args: [...WEBGPU_LAUNCH_ARGS],
        },
      },
    },
  ],
  webServer: {
    command: `node_modules/.bin/vite --port ${WORKER_WORKLOAD_PORT} --strictPort`,
    url: `http://127.0.0.1:${WORKER_WORKLOAD_PORT}`,
    reuseExistingServer: false,
    timeout: 120_000,
  },
});
