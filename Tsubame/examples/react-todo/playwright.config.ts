import { existsSync } from 'node:fs';
import { defineConfig, devices } from '@playwright/test';

// Claude Code on the web のリモート環境は Chromium を `/opt/pw-browsers/chromium` に
// 事前配置している（`playwright install` は不可）。その symlink があればそれを使い、
// 無ければ Playwright 管理のブラウザに委ねる（ローカル / CI）。
const PREINSTALLED_CHROMIUM = '/opt/pw-browsers/chromium';
const executablePath = existsSync(PREINSTALLED_CHROMIUM) ? PREINSTALLED_CHROMIUM : undefined;

/**
 * Playwright — react-todo の Miharashi e2e（#531：FW 非依存の実証）。
 *
 * solid の `examples/todo` と**同じ FW 非依存ホスト**（host.html / @miharashi/host-web）に、
 * react App Bundle を流し込んで描画されることを本物の Chromium で検証する。vite dev が
 * host.html を配信し、Miharashi 最小 dev server が react バンドルを HTTP 配信する。
 *
 * ポートは solid 版（5180 / 5181）と衝突しないよう 5182 / 5183 を既定にする。
 */
const PORT = Number(process.env.E2E_PORT ?? 5182);
// Miharashi 最小 dev server のポート（host.html が react バンドルを fetch する先）。
const MIHARASHI_DEV_PORT = Number(process.env.MIHARASHI_DEV_PORT ?? 5183);

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
      // Miharashi の最小 dev server。`vite build --watch` が react の単一 App Bundle を作り続け、
      // dev server がそれを HTTP 配信 + bundle 変更を WS で `reload` 中継する（full reload ループ・
      // ADR-0001）。初回ビルド完了は `/bundle.js` の 200 で待つ（それまでは 404）。host.html は
      // このポートからバンドルを fetch → eval し、reload WS を購読する（CORS は dev server が許可）。
      command: `MIHARASHI_DEV_PORT=${MIHARASHI_DEV_PORT} node scripts/miharashi-dev-server.mjs`,
      url: `http://localhost:${MIHARASHI_DEV_PORT}/bundle.js`,
      reuseExistingServer: !process.env.CI,
      timeout: 120_000,
    },
  ],
});
