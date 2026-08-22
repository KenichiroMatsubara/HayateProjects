import { expect, test, type Locator, type Page } from '@playwright/test';
import { Square } from 'tsshogi';

/**
 * 盤の操作を DOM Renderer 経路で確かめる e2e。`?renderer=dom` では 81 個のマスが本物の
 * DOM 要素なので、WASM も WebGPU も要らずに軽い CI ジョブで回せる。
 *
 * 盤の絵そのものは `draw` painter が `<canvas>` に描いていて DOM から覗けないため、
 * 局面の主張は `?debug=1` で生える `window.__shogiDebug` 越しに行う。
 */

const INITIAL_SFEN = 'lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1';

declare global {
  interface Window {
    __shogiDebug: {
      sfen(): string;
      lastMove(): string | null;
      ply(): number;
    };
  }
}

/** 盤のマス要素（screen order 0 = 左上）。9×9 grid の直接の子だけを取る。 */
function cells(page: Page): Locator {
  return page.locator('#dom-host div[style*="grid-template-columns"] > div');
}

/** 筋・段からマス要素を取る（盤は未反転の前提）。 */
function squareAt(page: Page, file: number, rank: number): Locator {
  return cells(page).nth(new Square(file, rank).index);
}

function debugState(page: Page) {
  return page.evaluate(() => ({
    sfen: window.__shogiDebug.sfen(),
    lastMove: window.__shogiDebug.lastMove(),
    ply: window.__shogiDebug.ply(),
  }));
}

// 盤の操作そのものを決定的に確かめたいので、既定の AI 対局ではなく人対人で開く。
// AI 対局は `ai-turn.spec.ts` が別に見る。
test.beforeEach(async ({ page }) => {
  await page.goto('/?renderer=dom&debug=1&opponent=human');
  await expect(cells(page)).toHaveCount(81);
});

test('初期局面が出て、81マスが盤として並ぶ', async ({ page }) => {
  expect(await debugState(page)).toEqual({ sfen: INITIAL_SFEN, lastMove: null, ply: 0 });
});

test('駒を選んで動かせる（7六歩 → 3四歩）', async ({ page }) => {
  await squareAt(page, 7, 7).click();
  await squareAt(page, 7, 6).click();
  await expect.poll(async () => (await debugState(page)).lastMove).toBe('7g7f');

  await squareAt(page, 3, 3).click();
  await squareAt(page, 3, 4).click();
  await expect.poll(async () => (await debugState(page)).lastMove).toBe('3c3d');
  expect((await debugState(page)).ply).toBe(2);
});

test('非合法な手は指せない（歩は2マス進めない）', async ({ page }) => {
  await squareAt(page, 7, 7).click();
  await squareAt(page, 7, 5).click();
  expect((await debugState(page)).ply, '局面は動かない').toBe(0);
});

test('相手の駒は動かせない', async ({ page }) => {
  await squareAt(page, 3, 3).click();
  await squareAt(page, 3, 4).click();
  expect((await debugState(page)).ply).toBe(0);
});

test('成り／不成を選べる', async ({ page }) => {
  // 飛車を 2八 → 2四 → 2三 と歩を取りながら進め、敵陣で成りを問わせる。
  for (const [from, to] of [
    [[2, 7], [2, 6]],
    [[3, 3], [3, 4]],
    [[2, 6], [2, 5]],
    [[8, 3], [8, 4]],
    [[2, 5], [2, 4]],
    [[8, 4], [8, 5]],
    [[2, 4], [2, 3]],
  ] as const) {
    await squareAt(page, from[0], from[1]).click();
    await squareAt(page, to[0], to[1]).click();
  }

  const prompt = page.getByText('成りますか？');
  await expect(prompt).toBeVisible();
  await page.getByText('成る', { exact: true }).click();
  await expect.poll(async () => (await debugState(page)).lastMove).toBe('2d2c+');
});

test('待ったで1手戻る', async ({ page }) => {
  await squareAt(page, 7, 7).click();
  await squareAt(page, 7, 6).click();
  await expect.poll(async () => (await debugState(page)).ply).toBe(1);

  await page.getByText('待った').click();
  await expect.poll(async () => (await debugState(page)).ply).toBe(0);
  expect((await debugState(page)).sfen).toBe(INITIAL_SFEN);
});

test('投了すると決着が出る', async ({ page }) => {
  await page.getByText('投了').click();
  await expect(page.getByText('投了 — 後手の勝ち')).toBeVisible();
});

test('新規対局で初期局面に戻る', async ({ page }) => {
  await squareAt(page, 7, 7).click();
  await squareAt(page, 7, 6).click();
  await expect.poll(async () => (await debugState(page)).ply).toBe(1);

  await page.getByText('新規対局').click();
  await expect.poll(async () => (await debugState(page)).sfen).toBe(INITIAL_SFEN);
});
