import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright — AI / 人間が「実ブラウザ」で動作確認するための E2E 設定。
 *
 * vitest + happy-dom（ユニット）は擬似 DOM なので canvas(vello/tiny-skia) の
 * 実描画や本物のレイアウトは検証できない。Playwright は本物の Chromium を
 * 起動し、`webServer` で vite dev を立ち上げてアプリ全体を駆動する。
 *
 * 既定は DOM レンダラー（`?renderer=dom`）を前提にしたスモーク。canvas 系は
 * WebGPU/WASM が要るため、別途ビルド済み wasm-pkgs と GPU 環境が必要。
 */
const PORT = Number(process.env.E2E_PORT ?? 5180);

export default defineConfig({
  testDir: './e2e',
  // ユニットテスト（src/**/*.test.ts, vitest）とは明確に分離する。
  testMatch: '**/*.spec.ts',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  use: {
    baseURL: `http://localhost:${PORT}`,
    // 失敗時のみ証跡を残す。AI が原因を見られるようにする。
    screenshot: 'only-on-failure',
    trace: 'on-first-retry',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  webServer: {
    command: `pnpm exec vite --port ${PORT} --strictPort`,
    url: `http://localhost:${PORT}`,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
