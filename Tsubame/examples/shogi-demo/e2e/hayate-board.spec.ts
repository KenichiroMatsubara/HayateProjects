import { expect, test, type Page } from '@playwright/test';

/**
 * Hayate Renderer 経路の盤。`?renderer=tiny-skia` は CPU ラスタライザなので
 * WebGPU の無い環境でも Canvas モードに入れる。
 *
 * **この spec でしか押さえられないもの**が 1 つある: 盤も駒も 1 枚の canvas に落ちるので、
 * DOM 経路の spec からは駒の**向き**が見えない。後手の駒が五角形も漢字も 180° 回って
 * いることは、ピクセルで見るしかない。ここが Phase P（駒を 1 つの描画単位にした）の
 * 唯一の実地の証拠になる。
 *
 * WASM 未ビルドやバックエンド初期化不可の環境では描画されないので理由付きで skip する
 * （DOM 経路の spec は常に走る）。
 *
 * 注意: canvas が空白のまま skip し続ける場合、まず ANGLE バックエンドを疑うこと。
 * `--use-angle=vulkan` は環境によって WebGPU canvas を恒久的に空白にする
 * （playwright.config.ts の `ANGLE` を参照）。
 */

/**
 * 盤の一部（木地 or 外枠）か。
 *
 * **木地だけで矩形を取ってはいけない**: 外枠は枡目の内側へ描き込まれるので、木地の
 * 外接矩形は枠の太さぶん内側にずれる。そのずれた矩形から `side / 9` でマスを割ると、
 * 全マスの標本位置が段ごとに少しずつずれて、測っているものが別物になる。
 * 枠（BOARD_EDGE ≈ rgb(103, 77, 50)）まで含めた外接矩形が盤の真の矩形で、
 * `board-metrics.ts` の `grid`（= `board`）と一致する。
 */
const isBoard = (r: number, g: number, b: number): boolean => {
  const wood = r > 200 && r < 245 && g > 180 && g < 220 && b > 120 && b < 180;
  const edge = r > 85 && r < 125 && g > 60 && g < 95 && b > 35 && b < 70;
  return wood || edge;
};

/** 駒の地色（PIECE_FACE ≈ rgb(246, 229, 191)）に十分近いか。 */
const isFace = (r: number, g: number, b: number): boolean =>
  r > 238 && g > 218 && g < 242 && b > 175 && b < 210;

interface BoardGeometry {
  /** canvas ピクセル座標での盤の左上と一辺。 */
  readonly x: number;
  readonly y: number;
  readonly side: number;
}

/**
 * 盤とみなす最小の一辺（canvas px）。レンダリング途中の断片を「盤が出た」と
 * 読んでしまわないための下限。盤は shell の maxWidth 520 まで広がるので、
 * これを下回るのは「まだ描き終わっていない」ときだけ。
 */
const MIN_BOARD_SIDE = 200;

/** canvas を読んで盤（木地＋外枠）の矩形を割り出す。 */
async function boardGeometry(page: Page): Promise<BoardGeometry | null> {
  return page.evaluate(
    ([boardSrc]) => {
      const onBoard = new Function('r', 'g', 'b', `return (${boardSrc})(r, g, b)`) as (
        r: number,
        g: number,
        b: number,
      ) => boolean;
      const c = document.getElementById('canvas-stage') as HTMLCanvasElement | null;
      if (!c || c.width === 0) return null;
      const t = document.createElement('canvas');
      t.width = c.width;
      t.height = c.height;
      const ctx = t.getContext('2d');
      if (!ctx) return null;
      try {
        ctx.drawImage(c, 0, 0);
      } catch {
        return null;
      }
      const { data } = ctx.getImageData(0, 0, c.width, c.height);
      let x0 = Infinity;
      let y0 = Infinity;
      let x1 = -1;
      let y1 = -1;
      for (let y = 0; y < c.height; y++) {
        for (let x = 0; x < c.width; x++) {
          const i = (y * c.width + x) * 4;
          if (onBoard(data[i]!, data[i + 1]!, data[i + 2]!)) {
            if (x < x0) x0 = x;
            if (x > x1) x1 = x;
            if (y < y0) y0 = y;
            if (y > y1) y1 = y;
          }
        }
      }
      if (x1 < 0) return null;
      return { x: x0, y: y0, side: x1 - x0 + 1 };
    },
    [isBoard.toString()],
  );
}

/**
 * 1 マス分の駒の「重心の高さ」を測る。五角形は頭が尖っているので、駒の地色が最も
 * 広い行は**足元側**に寄る。返すのはマス高さに対する比（0 = 上端, 1 = 下端）。
 * 駒が無いマスは `null`。
 */
async function pieceWeightY(
  page: Page,
  geo: BoardGeometry,
  col: number,
  row: number,
): Promise<number | null> {
  return page.evaluate(
    ([faceSrc, g, c, r]) => {
      const face = new Function('r', 'g', 'b', `return (${faceSrc})(r, g, b)`) as (
        r: number,
        g: number,
        b: number,
      ) => boolean;
      const geo = g as { x: number; y: number; side: number };
      const canvas = document.getElementById('canvas-stage') as HTMLCanvasElement;
      const t = document.createElement('canvas');
      t.width = canvas.width;
      t.height = canvas.height;
      const ctx = t.getContext('2d')!;
      ctx.drawImage(canvas, 0, 0);
      const { data } = ctx.getImageData(0, 0, canvas.width, canvas.height);
      const cell = geo.side / 9;
      const x0 = Math.round(geo.x + (c as number) * cell);
      const y0 = Math.round(geo.y + (r as number) * cell);
      const size = Math.round(cell);

      // 各行の駒地色の幅を数え、幅で重み付けした平均の y を出す。
      let weighted = 0;
      let total = 0;
      for (let y = 0; y < size; y++) {
        let width = 0;
        for (let x = 0; x < size; x++) {
          const i = ((y0 + y) * canvas.width + (x0 + x)) * 4;
          if (face(data[i]!, data[i + 1]!, data[i + 2]!)) width++;
        }
        weighted += width * y;
        total += width;
      }
      if (total < size) return null; // 駒が無い（or ほぼ無い）マス
      return weighted / total / size;
    },
    [isFace.toString(), geo, col, row] as const,
  );
}

test.describe('将棋盤 — Hayate Renderer 経路（tiny-skia CPU backend）', () => {
  test('盤と駒が canvas に描かれ、後手の駒は 180° 回っている', async ({ page }) => {
    test.setTimeout(90_000);

    const bootErrors: string[] = [];
    page.on('console', (msg) => {
      if (msg.type() === 'error') bootErrors.push(msg.text());
    });
    page.on('pageerror', (err) => bootErrors.push(err.message));

    await page.goto('/?renderer=tiny-skia&debug=1&opponent=human');

    // 盤が「出た」だけでなく「出揃った」まで待つ。最初の非 null で進むと、
    // 描画途中の断片を盤と誤認して座標が全部ずれる。
    let geo: BoardGeometry | null = null;
    await expect
      .poll(
        async () => {
          geo = await boardGeometry(page);
          return geo !== null && geo.side >= MIN_BOARD_SIDE;
        },
        { timeout: 30_000 },
      )
      .toBe(true)
      .catch(() => {
        /* 未描画のまま — 下で skip 判定する。 */
      });

    test.skip(
      geo === null || geo.side < MIN_BOARD_SIDE,
      'Hayate canvas 経路が描画されない（WASM 未ビルド / バックエンド初期化不可 / ' +
        'ANGLE バックエンドが不適合）。`pnpm --filter hayate build` 済みの環境で走る。' +
        (bootErrors.length ? ` boot errors: ${bootErrors.slice(0, 3).join(' | ')}` : ''),
    );
    const board = geo!;

    // 盤は 9 マスに割り切れる大きさで出揃っている。
    expect(board.side).toBeGreaterThanOrEqual(MIN_BOARD_SIDE);

    // 後手の香車（9一 = col 0, row 0）と先手の香車（9九 = col 0, row 8）を比べる。
    // 同じ駒種なので、違うのは向きだけ。
    const gote = await pieceWeightY(page, board, 0, 0);
    const sente = await pieceWeightY(page, board, 0, 8);
    expect(gote, '9一に後手の駒がある').not.toBeNull();
    expect(sente, '9九に先手の駒がある').not.toBeNull();

    // 先手（頭が上）は地色の重心が下寄り、後手（頭が下）は上寄りになる。
    expect(sente!, '先手の駒は頭が上（重心は下寄り）').toBeGreaterThan(0.5);
    expect(gote!, '後手の駒は頭が下（重心は上寄り）').toBeLessThan(0.5);

    // さらに強い主張: `rotate(π)` はマス中心まわりの回転なので、後手の重心は先手の
    // **鏡像**（`1 - sente`）になる。任意に決めた差の閾値ではなく、回転が実際に持つ
    // 性質そのものを見る。回転を外すと両者が同じ側に来てここが落ちる。
    expect(Math.abs(gote! - (1 - sente!)), '後手は先手の鏡像であるべき').toBeLessThan(0.03);
  });

  test('canvas を叩いて駒が動く（当たり判定が Hayate 経路でも通る）', async ({ page }) => {
    test.setTimeout(90_000);
    await page.goto('/?renderer=tiny-skia&debug=1&opponent=human');

    let geo: BoardGeometry | null = null;
    await expect
      .poll(
        async () => {
          geo = await boardGeometry(page);
          return geo !== null && geo.side >= MIN_BOARD_SIDE;
        },
        { timeout: 30_000 },
      )
      .toBe(true)
      .catch(() => {});
    test.skip(geo === null || geo.side < MIN_BOARD_SIDE, 'Hayate canvas 経路が描画されない');
    const board = geo!;

    const rect = await page.locator('#canvas-stage').boundingBox();
    const dpr = await page.evaluate(() => {
      const c = document.getElementById('canvas-stage') as HTMLCanvasElement;
      return c.width / c.getBoundingClientRect().width;
    });
    const cell = board.side / 9;
    // 筋・段（7七 → 7六）を画面座標へ。列は 9 - file、行は rank - 1。
    const at = (file: number, rank: number) => ({
      x: rect!.x + (board.x + (9 - file + 0.5) * cell) / dpr,
      y: rect!.y + (board.y + (rank - 1 + 0.5) * cell) / dpr,
    });

    const from = at(7, 7);
    const to = at(7, 6);
    await page.mouse.click(from.x, from.y);
    await page.waitForTimeout(200);
    await page.mouse.click(to.x, to.y);

    await expect
      .poll(() => page.evaluate(() => window.__shogiDebug.lastMove()), { timeout: 10_000 })
      .toBe('7g7f');
  });
});
