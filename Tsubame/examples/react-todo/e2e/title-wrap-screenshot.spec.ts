import { expect, test } from '@playwright/test';

/**
 * 診断用: React TODO のタイトル行を DOM / Hayate(tiny-skia) 両モードで撮り比べる。
 * 「react が hayate だけタイトルが異常に折れ曲がる」症状の再現確認に使う。
 */

const SHOT_DIR = 'e2e/__screens__';

test('DOM renderer: タイトル行のスクショ', async ({ page }) => {
  await page.addInitScript(() => window.localStorage.clear());
  await page.goto('/?renderer=dom');
  await page.waitForTimeout(800);
  await page.screenshot({ path: `${SHOT_DIR}/react-dom.png` });
});

test('Hayate tiny-skia renderer: タイトル行のスクショ', async ({ page }) => {
  await page.addInitScript(() => window.localStorage.clear());
  await page.goto('/?renderer=tiny-skia');
  // Canvas モードは WASM ロード + 初回フレーム待ち。
  await page.waitForTimeout(2500);
  const canvas = page.locator('#canvas-stage');
  await expect(canvas).toBeVisible();
  await page.screenshot({ path: `${SHOT_DIR}/react-tinyskia.png` });
});
