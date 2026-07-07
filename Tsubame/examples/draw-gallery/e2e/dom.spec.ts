import { expect, test, type Locator } from '@playwright/test';

/**
 * DOM Renderer 経路の e2e（issue #732）。`?renderer=dom` でギャラリーを起動し、
 * 各サンプル painter が敷いた draw `<canvas>`（Tsubame ADR-0014）に実際に描画が
 * 現れる（空白でない）ことを本物の Chromium で確認する。DOM 経路は canvas 2D の
 * replay なので WebGPU / WASM 不要 — CI のヘッドレスでそのまま走る。
 */

/** canvas 要素の、アルファ > 0（＝描かれた）ピクセル数を数える。 */
async function paintedPixelCount(canvas: Locator): Promise<number> {
  return canvas.evaluate((el) => {
    const c = el as HTMLCanvasElement;
    const ctx = c.getContext('2d');
    if (!ctx || c.width === 0 || c.height === 0) return 0;
    const { data } = ctx.getImageData(0, 0, c.width, c.height);
    let painted = 0;
    for (let i = 3; i < data.length; i += 4) if (data[i]! > 0) painted++;
    return painted;
  });
}

test.describe('Draw Gallery — DOM renderer', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/?renderer=dom');
  });

  test('ギャラリーの見出しと全サンプル painter が描画される', async ({ page }) => {
    // DOM 経路は text を本物の DOM テキストへ落とす。
    await expect(page.getByText('Draw Gallery')).toBeVisible();

    // 5 種のサンプル + サイズ追従デモ = 6 枚の draw canvas。
    const canvases = page.locator('#dom-host canvas');
    await expect
      .poll(async () => canvases.count(), { timeout: 10_000 })
      .toBeGreaterThanOrEqual(6);

    // どの draw canvas も空白でない（何かが描かれている）。
    const count = await canvases.count();
    for (let i = 0; i < count; i++) {
      const painted = await paintedPixelCount(canvases.nth(i));
      expect(painted, `draw canvas #${i} should be non-blank`).toBeGreaterThan(0);
    }
  });

  test('サイズ追従デモ: box を大きくすると painter が描き直してカバレッジが増える', async ({
    page,
  }) => {
    // 最後の draw canvas がサイズ追従デモ（App の描画順で末尾）。
    const demo = page.locator('#dom-host canvas').last();
    await expect(demo).toBeVisible();

    await page.getByText('S', { exact: true }).click();
    await expect.poll(() => paintedPixelCount(demo), { timeout: 5_000 }).toBeGreaterThan(0);
    const small = await paintedPixelCount(demo);

    await page.getByText('L', { exact: true }).click();
    // resize→layout→paint を待つ。L は面積が大きくセル数も増えるので painted も増える。
    await expect.poll(() => paintedPixelCount(demo), { timeout: 5_000 }).toBeGreaterThan(small);
  });
});
