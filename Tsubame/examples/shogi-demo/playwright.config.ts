import { existsSync } from 'node:fs';
import { defineConfig, devices } from '@playwright/test';

// Claude Code on the web のリモート環境は Chromium を `/opt/pw-browsers/chromium` に
// 事前配置している（`playwright install` は不可）。その symlink があればそれを使い、
// 無ければ Playwright 管理のブラウザに委ねる（ローカル / CI）。
const PREINSTALLED_CHROMIUM = '/opt/pw-browsers/chromium';
const executablePath = existsSync(PREINSTALLED_CHROMIUM) ? PREINSTALLED_CHROMIUM : undefined;

/**
 * Playwright — shogi-demo の Torimi e2e（#531：FW 非依存の実証）。
 *
 * solid / react の各 example と**同じ FW 非依存ホスト**（host.html / @torimi/host-web）に、
 * shogi App Bundle を流し込んで描画されることを本物の Chromium で検証する。vite dev が
 * host.html を配信し、Torimi 最小 dev server が shogi バンドルを HTTP 配信する。
 *
 * ポートは solid 版（5180 / 5181）・react 版（5182 / 5183）と衝突しないよう
 * 5184 / 5185 を既定にする。
 */
const PORT = Number(process.env.E2E_PORT ?? 5184);
// Torimi 最小 dev server のポート（host.html が shogi バンドルを fetch する先）。
const TORIMI_DEV_PORT = Number(process.env.TORIMI_DEV_PORT ?? 5185);

// ANGLE バックエンド。既定は `gl`（draw-gallery の playwright.config.ts と同じ理由）。
// native Vulkan ICD が不安定な環境では `vulkan` を選ぶと WebGPU canvas が恒久的に空白に
// なり（`A valid external Instance reference no longer exists.`）、要素は描かれて canvas の
// 中身だけ出ないので**コード側の問題に見える**。`gl` は ICD を迂回するだけで WebGPU の
// 機能は変わらない。`E2E_ANGLE=vulkan` で戻せる。
const ANGLE = process.env.E2E_ANGLE ?? 'gl';

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
      use: {
        ...devices['Desktop Chrome'],
        launchOptions: {
          executablePath,
          // Hayate Renderer 経路（`hayate-board.spec.ts`）が WebGPU / CPU バックエンドへ
          // 入れるようにする。DOM 経路には無害。
          args: ['--enable-unsafe-webgpu', '--ignore-gpu-blocklist', `--use-angle=${ANGLE}`],
        },
      },
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
