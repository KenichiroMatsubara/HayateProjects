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
// Torimi 最小 dev server のポート（host.html がバンドルを fetch する先）。
const TORIMI_DEV_PORT = Number(process.env.TORIMI_DEV_PORT ?? 5181);

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
      // Torimi CLI の web dev。`torimi dev web` が build（vite）→ 配信（@torimi/dev-server）→
      // bundle 変更を WS で `reload` 中継まで面倒を見る（full reload ループ・ADR-0008）。初回ビルド
      // 完了は `/bundle.js` の 200 で待つ（それまでは 404）。host.html はこのポートからバンドルを
      // fetch → eval し、reload WS を購読する（CORS は dev server が許可）。
      command: `TORIMI_DEV_PORT=${TORIMI_DEV_PORT} pnpm exec torimi dev web`,
      url: `http://localhost:${TORIMI_DEV_PORT}/bundle.js`,
      reuseExistingServer: !process.env.CI,
      timeout: 120_000,
    },
  ],
});
