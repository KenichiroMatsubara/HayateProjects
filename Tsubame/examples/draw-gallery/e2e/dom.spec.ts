import { expect, test, type Locator } from '@playwright/test';
import { GALLERY_PAINTERS } from '../src/painters.js';

/**
 * DOM Renderer 経路の e2e（issue #732）。`?renderer=dom` でギャラリーを起動し、
 * 各サンプル painter が敷いた draw `<canvas>`（Tsubame ADR-0014）に実際に描画が
 * 現れる（空白でない）ことを本物の Chromium で確認する。DOM 経路は canvas 2D の
 * replay なので WebGPU / WASM 不要 — CI のヘッドレスでそのまま走る。
 */

/**
 * カード canvas の index。App は `GALLERY_PAINTERS` を順に map し、その後ろに
 * サイズ追従デモの canvas を 1 枚置く。順序の正本を e2e にコピーせず、
 * App が map するのと同じ配列から引く。
 */
function cardIndex(id: string): number {
  const index = GALLERY_PAINTERS.findIndex((p) => p.id === id);
  if (index < 0) throw new Error(`no gallery painter with id ${id}`);
  return index;
}

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

    // サンプル painter 各 1 枚 + サイズ追従デモ 1 枚。
    const canvases = page.locator('#dom-host canvas');
    await expect
      .poll(async () => canvases.count(), { timeout: 10_000 })
      .toBeGreaterThanOrEqual(GALLERY_PAINTERS.length + 1);

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

/**
 * テキスト描画（PRD #723 / ADR-0141）。`drawText` が本当にグリフを出しているかを、
 * 「空白でない」より強い主張で見る — 非空白だけなら塗り 1 個でも通ってしまう。
 */
test.describe('Draw Gallery — DOM renderer, drawText', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/?renderer=dom');
  });

  test('書体見本: 行ごとに分かれた複数の水平帯にインクが乗る', async ({ page }) => {
    const canvas = page.locator('#dom-host canvas').nth(cardIndex('text-sampler'));
    await expect(canvas).toBeVisible();

    // インクのある行を数える。1 行に潰れていたら「見本」になっていない。
    await expect
      .poll(
        () =>
          canvas.evaluate((el) => {
            const c = el as HTMLCanvasElement;
            const ctx = c.getContext('2d');
            if (!ctx || c.width === 0) return 0;
            const { data } = ctx.getImageData(0, 0, c.width, c.height);
            const rowHasInk: boolean[] = [];
            for (let y = 0; y < c.height; y++) {
              let ink = false;
              for (let x = 0; x < c.width && !ink; x++) {
                if (data[(y * c.width + x) * 4 + 3]! > 0) ink = true;
              }
              rowHasInk.push(ink);
            }
            // 連続したインク行を 1 帯として数える。
            let bands = 0;
            for (let y = 0; y < rowHasInk.length; y++) {
              if (rowHasInk[y] && !rowHasInk[y - 1]) bands++;
            }
            return bands;
          }),
        { timeout: 10_000 },
      )
      .toBeGreaterThanOrEqual(3);
  });

  test('回転テキスト: グリフが中心の周りに輪状に散り、中心は空く', async ({ page }) => {
    const canvas = page.locator('#dom-host canvas').nth(cardIndex('rotated-text'));
    await expect(canvas).toBeVisible();

    // 4 象限すべてにインクがあり、かつ中心の小領域は空。回転が効かず
    // 全部が同じ場所に描かれていれば、この形にはならない。
    await expect
      .poll(
        () =>
          canvas.evaluate((el) => {
            const c = el as HTMLCanvasElement;
            const ctx = c.getContext('2d');
            if (!ctx || c.width === 0) return null;
            const { data } = ctx.getImageData(0, 0, c.width, c.height);
            const alphaAt = (x: number, y: number): number =>
              data[(y * c.width + x) * 4 + 3]!;
            const inkIn = (x0: number, x1: number, y0: number, y1: number): boolean => {
              for (let y = y0; y < y1; y++) {
                for (let x = x0; x < x1; x++) if (alphaAt(x, y) > 0) return true;
              }
              return false;
            };
            const mx = Math.floor(c.width / 2);
            const my = Math.floor(c.height / 2);
            const quadrants = [
              inkIn(0, mx, 0, my),
              inkIn(mx, c.width, 0, my),
              inkIn(0, mx, my, c.height),
              inkIn(mx, c.width, my, c.height),
            ];
            const r = Math.floor(Math.min(c.width, c.height) * 0.06);
            const centreClear = !inkIn(mx - r, mx + r, my - r, my + r);
            return { quadrants: quadrants.filter(Boolean).length, centreClear };
          }),
        { timeout: 10_000 },
      )
      .toEqual({ quadrants: 4, centreClear: true });
  });
});
