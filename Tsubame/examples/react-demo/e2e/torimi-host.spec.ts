import { expect, test } from '@playwright/test';

/**
 * Torimi の FW 非依存 e2e（#531 / ADR-0001）。solid（examples/solid-demo）と**同じ FW 非依存ホスト**
 * （host.html / @torimi/host-web）が、HTTP 配信された **react** App Bundle を fetch → eval し、
 * `createHayateWebHost` で canvas 上に host bootstrap を確立してバンドルの mount に渡す。ホスト側に
 * react 固有のコードは一切無く、react は流し込むバンドルが持ち込む。
 *
 * 「Viewer 一本で全 JS フレームワークが動く」ことを、solid のテスト（examples/solid-demo の
 * torimi-host.spec）と同型の描画証明で本物の Chromium に対して確かめる。
 */

const TORIMI_DEV_PORT = Number(process.env.TORIMI_DEV_PORT ?? 5183);
const DEV_SERVER_URL = `http://localhost:${TORIMI_DEV_PORT}`;

test.describe('Torimi host — renders the HTTP-served react bundle', () => {
  test('ホストページでreact-sketchへpointer gestureを送れる', async ({ page }) => {
    test.setTimeout(60_000);
    await page.goto(`/host.html?dev=${encodeURIComponent(DEV_SERVER_URL)}`);

    // fetch → eval → createHayateWebHost → mount が端から端まで貫けたこと（react バンドルでも
    // ホストは無改造）。data 属性は FW 非依存ホスト（host-boot.ts）が立てる。
    await expect(page.locator('html')).toHaveAttribute('data-torimi-status', 'mounted', {
      timeout: 30_000,
    });

    const canvas = page.locator('#torimi-canvas');
    await expect(canvas).toBeVisible();

    // surface 上にレンダラが初期化され backing store が確保されたこと。
    await expect
      .poll(async () => canvas.evaluate((el) => (el as HTMLCanvasElement).width))
      .toBeGreaterThan(0);

    // EditContext 非対応なら Canvas モードに入れないため skip。
    const editContextSupported = await page.evaluate(
      () => typeof (globalThis as { EditContext?: unknown }).EditContext !== 'undefined',
    );
    test.skip(!editContextSupported, 'EditContext 非対応ブラウザ（Canvas モードに入れない）');

    await page.waitForTimeout(300);
    const box = await canvas.boundingBox();
    expect(box, 'canvas bounding box').not.toBeNull();
    if (!box) return;

    await page.mouse.move(box.x + box.width * 0.25, box.y + box.height * 0.4);
    await page.mouse.down();
    await page.mouse.move(box.x + box.width * 0.7, box.y + box.height * 0.7, { steps: 12 });
    await page.mouse.up();

    await expect(page.locator('html')).toHaveAttribute('data-torimi-status', 'mounted');
    await expect(canvas).toHaveCSS('cursor', 'crosshair');
  });
});
