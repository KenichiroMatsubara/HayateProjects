import { existsSync } from 'node:fs';
import { defineConfig, devices } from '@playwright/test';

// #697 専用の Playwright 設定。既定の `playwright.config.ts`（DOM レンダラー中心のスモーク群）
// とは別出しにしてある: このスペックは (1) `layer-present` feature ON の追加 WASM ビルド
// （`pnpm --filter hayate build:layer-present` → `Hayate/wasm-pkgs/pkg-layer-present`）を要求し、
// (2) 実 GPU（`navigator.gpu.requestAdapter()`）が要る診断的スペックで、他のスモークと同じ
// webServer 起動列に混ぜると「pkg-layer-present 未ビルド」で無関係なテストまで巻き込んで
// 待たせる／落とすため。
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

// OFF: 既定ビルド（`Hayate/wasm-pkgs/pkg`、layer-present feature OFF）を素の vite.config.ts で配信。
const OFF_PORT = Number(process.env.E2E_LAYER_PRESENT_OFF_PORT ?? 5185);
// ON: `vite.config.e2e-layer-present.ts`（`hayate-adapter-web` を pkg-layer-present へ alias）で配信。
const ON_PORT = Number(process.env.E2E_LAYER_PRESENT_ON_PORT ?? 5186);

export default defineConfig({
  testDir: './e2e',
  testMatch: 'layer-present-webgpu.spec.ts',
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: 0,
  timeout: 60_000,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  use: {
    screenshot: 'only-on-failure',
    trace: 'retain-on-failure',
  },
  projects: [
    {
      name: 'chromium-webgpu',
      use: {
        ...devices['Desktop Chrome'],
        launchOptions: {
          executablePath,
          args: ['--enable-unsafe-webgpu', '--ignore-gpu-blocklist', '--use-angle=vulkan'],
        },
      },
    },
  ],
  webServer: [
    {
      command: `pnpm exec vite --port ${OFF_PORT} --strictPort`,
      url: `http://localhost:${OFF_PORT}`,
      reuseExistingServer: !process.env.CI,
      timeout: 120_000,
    },
    {
      command: `pnpm exec vite --config vite.config.e2e-layer-present.ts --port ${ON_PORT} --strictPort`,
      url: `http://localhost:${ON_PORT}`,
      reuseExistingServer: !process.env.CI,
      timeout: 120_000,
    },
  ],
});
