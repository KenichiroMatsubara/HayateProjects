import { existsSync } from 'node:fs';
import { defineConfig, devices } from '@playwright/test';

// Claude Code on the web のリモート環境は Chromium を `/opt/pw-browsers/chromium` に
// 事前配置している（`playwright install` は不可）。その symlink があればそれを使い、
// 無ければ Playwright 管理のブラウザに委ねる（examples/solid-demo と同じ流儀）。
const PREINSTALLED_CHROMIUM = '/opt/pw-browsers/chromium';
const executablePath = existsSync(PREINSTALLED_CHROMIUM) ? PREINSTALLED_CHROMIUM : undefined;

const PORT = Number(process.env.E2E_PORT ?? 5190);

// ANGLE バックエンド。既定は `gl`。
//
// `vulkan` を選ぶと、native Vulkan ICD が不安定な環境（Mesa/amdgpu 等）では WebGPU の
// canvas が恒久的に空白になる（コンソールに
// `OperationError: A valid external Instance reference no longer exists.`）。
// 要素やレイアウトは描かれて canvas の中身だけ出ないので、**「Hayate 経路は動かない」と
// 読み違えやすい** — 実際 2026-08-22 にそれで誤った結論を出した。この spec は描かれ
// なければ skip する作りなので、症状は「常に skip」という静かな形でも出る。
//
// `gl` は ANGLE 経由で ICD を迂回するだけで WebGPU の機能は同じなので、既定はこちらに
// する。vulkan 側を確かめたいときは `E2E_ANGLE=vulkan`。
const ANGLE = process.env.E2E_ANGLE ?? 'gl';

export default defineConfig({
  testDir: './e2e',
  testMatch: '**/*.spec.ts',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
  use: {
    baseURL: `http://localhost:${PORT}`,
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
          // tiny-skia CPU backend は WebGPU 不要だが、vello 経路の手動確認や将来の
          // GPU 有効 CI 用に WebGPU フラグも付けておく（無害）。
          args: ['--enable-unsafe-webgpu', '--ignore-gpu-blocklist', `--use-angle=${ANGLE}`],
        },
      },
    },
  ],
  webServer: {
    command: `pnpm exec vite --port ${PORT} --strictPort`,
    url: `http://localhost:${PORT}`,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
