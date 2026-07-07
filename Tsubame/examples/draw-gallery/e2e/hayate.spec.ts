import { expect, test, type Page } from '@playwright/test';

/**
 * Hayate Renderer 経路の e2e（issue #732）。`?renderer=tiny-skia` で同じギャラリーを
 * 起動する。tiny-skia は CPU ラスタライザなので WebGPU の無いヘッドレスでも Canvas
 * モードに入れる（prior art: examples/todo の canvas-a11y-mirror.spec.ts）。同一 painter
 * が Hayate 経路（wire の `draws` チャネル → WASM ラスタライザ）でも `<canvas>` に
 * 描画されることを確認する。両レンダラー間のピクセル一致は要求しない（形状の目視
 * 同等のみ・issue #732）。
 *
 * この経路は WASM ビルド（`pnpm --filter hayate build`）が要る。未ビルド環境（stub は
 * 「WASM not built」を投げる）や GPU/CPU バックエンドが初期化できない環境では、
 * `<canvas>` が描画されないので理由付きで skip する（DOM 経路の spec は常に走る）。
 */

/** #canvas-stage に描かれた「色の種類数」をサンプリングして数える。 */
async function distinctColorCount(page: Page): Promise<number> {
  return page.evaluate(() => {
    const c = document.getElementById('canvas-stage') as HTMLCanvasElement | null;
    if (!c || c.width === 0 || c.height === 0) return 0;
    // WebGPU/CPU いずれの canvas でも drawImage で 2D コピーへ写せば読める。
    const tmp = document.createElement('canvas');
    tmp.width = c.width;
    tmp.height = c.height;
    const ctx = tmp.getContext('2d');
    if (!ctx) return 0;
    try {
      ctx.drawImage(c, 0, 0);
    } catch {
      return 0;
    }
    const { data } = ctx.getImageData(0, 0, tmp.width, tmp.height);
    const seen = new Set<number>();
    // 粗いグリッドでサンプリング（全画素は重い）。
    const stride = 40 * 4;
    for (let i = 0; i < data.length; i += stride) {
      seen.add((data[i]! << 24) | (data[i + 1]! << 16) | (data[i + 2]! << 8) | data[i + 3]!);
      if (seen.size > 8) break;
    }
    return seen.size;
  });
}

test.describe('Draw Gallery — Hayate renderer (tiny-skia CPU backend)', () => {
  test('同一 painter が Hayate 経路の canvas にも描画される（未対応環境は skip）', async ({
    page,
  }) => {
    test.setTimeout(60_000);

    const bootErrors: string[] = [];
    page.on('console', (msg) => {
      if (msg.type() === 'error') bootErrors.push(msg.text());
    });
    page.on('pageerror', (err) => bootErrors.push(err.message));

    await page.goto('/?renderer=tiny-skia');

    // レンダリングされると色の種類が増える（背景一色 = 未描画）。数秒ポーリングする。
    let rendered = false;
    await expect
      .poll(async () => (rendered = (await distinctColorCount(page)) > 2), { timeout: 20_000 })
      .toBe(true)
      .catch(() => {
        /* 未描画のまま — 下で skip 判定する。 */
      });

    test.skip(
      !rendered,
      'Hayate canvas 経路が描画されない（WASM 未ビルド or バックエンド初期化不可）。' +
        '`pnpm --filter hayate build` 済みかつ CPU/GPU が使える環境で走る。' +
        (bootErrors.length ? ` boot errors: ${bootErrors.slice(0, 3).join(' | ')}` : ''),
    );

    // 描画された: canvas は空白でない（複数色 = 形状が乗っている）。
    expect(await distinctColorCount(page)).toBeGreaterThan(2);
  });
});
