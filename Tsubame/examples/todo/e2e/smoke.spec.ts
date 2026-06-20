import { expect, test } from '@playwright/test';

/**
 * 実ブラウザでのスモーク。DOM レンダラー（`?renderer=dom`）でアプリを起動し、
 * 「描画される / 入力できる / 追加が反映される」までを本物の Chromium で確認する。
 *
 * AI はこのファイルを雛形に、確認したい挙動の spec を足していく。
 */
test.describe('Tsubame Task Studio — DOM renderer', () => {
  test.beforeEach(async ({ page }) => {
    // localStorage の持ち越しを避けるため、起動のたびに seed から始める。
    await page.addInitScript(() => window.localStorage.clear());
    await page.goto('/?renderer=dom');
  });

  test('seed タスクが描画される', async ({ page }) => {
    // DOM レンダラーは text-input を本物の <input> に落とす。
    await expect(page.locator('input[placeholder="新しいタスクを入力…"]')).toBeVisible();
    // seed データ（todo-model.ts の SEED）が見えること。
    await expect(page.getByText('レイアウトエンジンに flex-wrap を実装')).toBeVisible();
    await expect(page.getByText('ダークモードの配色を調整')).toBeVisible();
  });

  test('新しいタスクを追加すると反映される', async ({ page }) => {
    const input = page.locator('input[placeholder="新しいタスクを入力…"]');
    await input.fill('Playwright から追加したタスク');
    await input.press('Enter');

    await expect(page.getByText('Playwright から追加したタスク')).toBeVisible();
  });
});
