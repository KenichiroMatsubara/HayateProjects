import { expect, test, type Page } from '@playwright/test';
import { Square } from 'tsshogi';

/**
 * AI 対局の e2e。ここで一番確かめたいのは強さではなく、
 * **AI が考えている間もフレームが回り続けること**（`runEngineSearch` が
 * `setTimeout` でスライスを刻む設計が、実ブラウザで効いていること）。
 */

declare global {
  interface Window {
    __shogiDebug: { sfen(): string; lastMove(): string | null; ply(): number };
    __rafTicks: number;
  }
}

function cells(page: Page) {
  return page.locator('#dom-host div[style*="grid-template-columns"] > div');
}

function squareAt(page: Page, file: number, rank: number) {
  return cells(page).nth(new Square(file, rank).index);
}

test.beforeEach(async ({ page }) => {
  // 既定（`opponent` 未指定）は AI が後手を持つ。
  await page.goto('/?renderer=dom&debug=1');
  await expect(cells(page)).toHaveCount(81);
});

test('人が指すと AI が応じる', async ({ page }) => {
  test.setTimeout(60_000);
  await squareAt(page, 7, 7).click();
  await squareAt(page, 7, 6).click();

  await page.waitForFunction(() => window.__shogiDebug.ply() >= 2, null, { timeout: 30_000 });
  const state = await page.evaluate(() => ({
    ply: window.__shogiDebug.ply(),
    sfen: window.__shogiDebug.sfen(),
  }));
  expect(state.ply).toBe(2);
  // 後手が指したので手番は先手に戻っている。
  expect(state.sfen).toContain(' b ');
});

test('AI の思考中もフレームが回り続ける', async ({ page }) => {
  test.setTimeout(60_000);
  await page.evaluate(() => {
    window.__rafTicks = 0;
    const tick = (): void => {
      window.__rafTicks += 1;
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  });

  await squareAt(page, 7, 7).click();
  await squareAt(page, 7, 6).click();
  const before = await page.evaluate(() => window.__rafTicks);

  await page.waitForFunction(() => window.__shogiDebug.ply() >= 2, null, { timeout: 30_000 });
  const after = await page.evaluate(() => window.__rafTicks);

  // 探索がメインスレッドを占有していれば rAF は止まる。刻めていれば何十回も回る。
  expect(after - before, 'AI の手番中にフレームが進んでいる').toBeGreaterThan(5);
});
