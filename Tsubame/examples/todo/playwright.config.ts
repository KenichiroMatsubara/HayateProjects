import { existsSync } from 'node:fs';
import { defineConfig, devices } from '@playwright/test';

// Claude Code on the web のリモート環境は Chromium を `/opt/pw-browsers/chromium` に
// 事前配置している（`playwright install` は不可）。その symlink があればそれを使い、
// 無ければ Playwright 管理のブラウザに委ねる（ローカル / CI）。
const PREINSTALLED_CHROMIUM = '/opt/pw-browsers/chromium';
const executablePath = existsSync(PREINSTALLED_CHROMIUM) ? PREINSTALLED_CHROMIUM : undefined;

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
// Miharashi 最小 dev server のポート（host.html がバンドルを fetch する先）。
const MIHARASHI_DEV_PORT = Number(process.env.MIHARASHI_DEV_PORT ?? 5181);

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
      use: { ...devices['Desktop Chrome'], launchOptions: { executablePath } },
    },
  ],
  webServer: [
    {
      command: `pnpm exec vite --port ${PORT} --strictPort`,
      url: `http://localhost:${PORT}`,
      reuseExistingServer: !process.env.CI,
      timeout: 120_000,
    },
    {
      // Miharashi の最小 dev server。単一 App Bundle をビルドしてから HTTP 配信する。
      // host.html はこのポートからバンドルを fetch → eval する（CORS は dev server が許可）。
      command: `pnpm run build:miharashi && MIHARASHI_DEV_PORT=${MIHARASHI_DEV_PORT} node scripts/miharashi-dev-server.mjs`,
      url: `http://localhost:${MIHARASHI_DEV_PORT}/bundle.js`,
      reuseExistingServer: !process.env.CI,
      timeout: 120_000,
    },
  ],
});
