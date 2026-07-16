import { existsSync } from 'node:fs';
import { defineConfig, devices } from '@playwright/test';

// Vello の layer-present 実ブラウザ検証専用設定。通常の E2E とは分離し、WebGPU を有効にした
// Chromium で同じ WASM の runtime flag (`?layerPresent=0/1`) を比較する。
//
// `--enable-unsafe-webgpu` / `--ignore-gpu-blocklist` / `--use-angle=vulkan` を試す（README 参照）。
// Playwright 管理の chromium（`playwright install`）がこの環境には無く、代わりに実 Chrome
// （`/opt/pw-browsers/chromium` か、無ければシステムの `google-chrome`）を使う。
const PREINSTALLED_CHROMIUM = '/opt/pw-browsers/chromium';
const SYSTEM_CHROME = '/usr/bin/google-chrome';
const executablePath = existsSync(PREINSTALLED_CHROMIUM)
  ? PREINSTALLED_CHROMIUM
  : existsSync(SYSTEM_CHROME)
    ? SYSTEM_CHROME
    : undefined;

const PORT = Number(process.env.E2E_LAYER_PRESENT_PORT ?? 5185);

export default defineConfig({
  testDir: './e2e',
  testMatch: 'layer-present-webgpu.spec.ts',
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: 0,
  timeout: 60_000,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  use: {
    ...devices['Desktop Chrome'],
    baseURL: `http://localhost:${PORT}`,
    screenshot: 'only-on-failure',
    trace: 'retain-on-failure',
  },
  projects: [
    {
      name: 'chromium-webgpu',
      use: {
        launchOptions: {
          executablePath,
          args: ['--enable-unsafe-webgpu', '--ignore-gpu-blocklist', '--use-angle=vulkan'],
        },
      },
    },
  ],
  webServer: {
    command: `./node_modules/.bin/vite --port ${PORT} --strictPort`,
    url: `http://localhost:${PORT}`,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
