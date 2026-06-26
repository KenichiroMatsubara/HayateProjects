import { expect, test } from '@playwright/test';

/**
 * Miharashi トレーサーバレット #1 の e2e（ADR-0001）。最小 dev server が HTTP 配信する
 * 単一 App Bundle を、Miharashi ホストページ（host.html）が fetch → eval し、
 * `createHayateWebHost` で canvas 上に host bootstrap を確立してバンドルの mount に渡す。
 * 「ホストページを開くと todo が描画される」を本物の Chromium で端から端まで検証する。
 *
 * ホスト側は framework / renderer-canvas を持たず、それらは eval するバンドルが持ち込む。
 * この e2e が緑なら、その構造（host bootstrap だけ → バンドルが FW+renderer-canvas）が
 * 実ブラウザで成立していることの証拠になる。
 */

const MIHARASHI_DEV_PORT = Number(process.env.MIHARASHI_DEV_PORT ?? 5181);
const DEV_SERVER_URL = `http://localhost:${MIHARASHI_DEV_PORT}`;

test.describe('Miharashi host — renders the HTTP-served Tsubame bundle', () => {
  test.beforeEach(async ({ page }) => {
    // localStorage の持ち越しを避け、seed todo から始める。
    await page.addInitScript(() => window.localStorage.clear());
  });

  test('ホストページを開くと todo が canvas に描画される', async ({ page }) => {
    test.setTimeout(60_000);
    await page.goto(`/host.html?dev=${encodeURIComponent(DEV_SERVER_URL)}`);

    // fetch → eval → createHayateWebHost → mount が端から端まで貫けたこと。
    await expect(page.locator('html')).toHaveAttribute('data-miharashi-status', 'mounted', {
      timeout: 30_000,
    });

    const canvas = page.locator('#miharashi-canvas');
    await expect(canvas).toBeVisible();

    // surface 上にレンダラが初期化され backing store が確保されたこと。
    await expect
      .poll(async () => canvas.evaluate((el) => (el as HTMLCanvasElement).width))
      .toBeGreaterThan(0);

    // 描画証明：tiny-skia Canvas は DOM テキストを持たないので、seed todo（選択可能テキスト）の
    // 上で I-beam（text）カーソルが出ることを以て「todo が描画された」とする
    // （canvas-text-cursor.spec と同じ手法 / ADR-0105）。EditContext 非対応なら Canvas モードに
    // 入れないため skip。
    const editContextSupported = await page.evaluate(
      () => typeof (globalThis as { EditContext?: unknown }).EditContext !== 'undefined',
    );
    test.skip(!editContextSupported, 'EditContext 非対応ブラウザ（Canvas モードに入れない）');

    await page.waitForTimeout(300);
    const box = await canvas.boundingBox();
    expect(box, 'canvas bounding box').not.toBeNull();
    if (!box) return;

    const cursors = new Set<string>();
    const cols = 5;
    const rows = 6;
    for (let r = 1; r < rows; r++) {
      for (let c = 1; c < cols; c++) {
        const x = box.x + (box.width * c) / cols;
        const y = box.y + (box.height * r) / rows;
        await page.mouse.move(x, y);
        const cursor = await canvas.evaluate((el) => (el as HTMLCanvasElement).style.cursor);
        if (cursor) cursors.add(cursor);
      }
    }

    expect(
      cursors.has('text'),
      `seed todo（選択可能テキスト）の上で I-beam（text）が出るべき。観測した cursor: ${[...cursors].join(', ')}`,
    ).toBe(true);
  });
});
