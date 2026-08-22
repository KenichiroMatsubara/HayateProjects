import { expect, test } from '@playwright/test';

/**
 * Torimi の FW 非依存 e2e（ADR-0001）。solid / react の各 example と**同じ FW 非依存ホスト**
 * （host.html / @torimi/host-web）が、HTTP 配信された **将棋** App Bundle を fetch → eval し、
 * `createHayateWebHost` で canvas 上に host bootstrap を確立してバンドルの mount に渡す。
 * ホスト側に将棋固有のコードも react 固有のコードも一切無い。
 *
 * 「Viewer 一本で全 JS フレームワーク・全アプリが動く」ことを、Sketch より遥かに重い
 * アプリで確かめる（react-demo の同名 spec と同型）。
 *
 * この経路は WASM ビルド（`pnpm --filter hayate run build:all`）が要る。未ビルド環境では
 * host bootstrap が stub に当たって `WorkerBootError` になるので、理由付きで skip する
 * （draw-gallery の Hayate 経路 spec と同じ流儀）。DOM 経路の spec は常に走る。
 */

const TORIMI_DEV_PORT = Number(process.env.TORIMI_DEV_PORT ?? 5185);
const DEV_SERVER_URL = `http://localhost:${TORIMI_DEV_PORT}`;

test.describe('Torimi host — renders the HTTP-served shogi bundle', () => {
  test('ホストページで将棋バンドルが mount され surface が確保される', async ({ page }) => {
    test.setTimeout(60_000);
    await page.goto(`/host.html?dev=${encodeURIComponent(DEV_SERVER_URL)}`);

    // fetch → eval → createHayateWebHost → mount が端から端まで貫けたこと
    // （将棋バンドルでもホストは無改造）。data 属性は FW 非依存ホスト（host-boot.ts）が立てる。
    const status = page.locator('html');
    await expect
      .poll(async () => status.getAttribute('data-torimi-status'), { timeout: 30_000 })
      .not.toBeNull();

    const settled = await status.getAttribute('data-torimi-status');
    test.skip(
      settled === 'error' &&
        ((await status.getAttribute('data-torimi-error')) ?? '').includes('WorkerBootError'),
      'Hayate host が起動できない（WASM 未ビルド or バックエンド初期化不可）。' +
        '実描画の確認は WASM ビルド済み環境で行う。',
    );

    await expect(status).toHaveAttribute('data-torimi-status', 'mounted', { timeout: 30_000 });

    const canvas = page.locator('#torimi-canvas');
    await expect(canvas).toBeVisible();

    // surface 上にレンダラが初期化され backing store が確保されたこと。
    await expect
      .poll(async () => canvas.evaluate((el) => (el as HTMLCanvasElement).width))
      .toBeGreaterThan(0);
  });
});
