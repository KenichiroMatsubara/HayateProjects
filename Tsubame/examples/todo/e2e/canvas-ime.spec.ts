import { expect, test } from '@playwright/test';

/**
 * 実ブラウザ回帰 (#392): Canvas モードで、編集可能な text-input に
 * フォーカスが無い間は EditContext を canvas へ装着しない。
 *
 * EditContext を canvas に装着することがモバイルのソフトキーボードを立ち上げる。
 * 旧実装は起動時に常時装着していたため、文字でも何でもないものをタップして
 * canvas がフォーカスを得ただけでキーボードが現れた。修正後は core が
 * `ime_wants_keyboard()`（focused_text_input ベース）で出すべき時だけ装着する。
 *
 * Canvas モードは EditContext 対応ブラウザ専用 (ADR-0016/0048)。未対応なら
 * アプリは DOM モードへ自動フォールバックするので、その場合はスキップする。
 */
test.describe('Canvas IME — keyboard gating (#392)', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => window.localStorage.clear());
    // tiny-skia CPU backend: WebGPU の無いヘッドレスでも Canvas モードに入れる。
    await page.goto('/?renderer=tiny-skia');
  });

  test('起動直後（text-input 未フォーカス）は EditContext を装着しない', async ({ page }) => {
    const canvas = page.locator('#canvas-stage');

    // EditContext 非対応環境ではアプリが DOM モードへ落ちるのでスキップ。
    const editContextSupported = await page.evaluate(
      () => typeof (globalThis as { EditContext?: unknown }).EditContext !== 'undefined',
    );
    test.skip(!editContextSupported, 'EditContext 非対応ブラウザ（DOM モード）');

    // Canvas モードで起動し、最初のフレームが回るまで待つ。
    await expect(canvas).toBeVisible();
    await page.waitForTimeout(200);

    // 何もフォーカスしていない＝キーボードを出すべきでない。旧実装はここで
    // editContext が常時装着されていた（!= null）。修正後は未装着。
    const attachedAtRest = await canvas.evaluate(
      (el) => (el as HTMLCanvasElement).editContext != null,
    );
    expect(attachedAtRest, '未フォーカス時は EditContext 未装着であるべき').toBe(false);
  });
});
