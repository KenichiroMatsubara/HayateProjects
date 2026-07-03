import { expect, test } from '@playwright/test';

/**
 * 実ブラウザ回帰: レンダースケール（ADR-0129）が変化するフレームをまたいで、canvas が
 * 一瞬でも空白（クリアされたまま）にならないこと（#666・#667・#669）。
 *
 * `RenderScaleGovernor` は「予算超過フレームが `PRESSURE_FRAMES_TO_DEGRADE`（=3）連続」で
 * 1 段劣化する（ADR-0129）。#666 修正前は、劣化を検知したフレームで `apply_render_scale`
 * （`canvas.set_width`/`set_height` を含む）を present の**後**に呼んでいたため、
 * `canvas.width`/`height` への代入が HTML5 仕様どおり即座にバッファをクリアし、そのフレームで
 * 描画した内容がそのまま消え、次の `render()` まで canvas が空白になっていた。
 *
 * 実タイミング（真のジャンク）に依存すると環境差でフレークするため、意図的に予算超過フレームを
 * 作る: wheel イベントを frame budget（60Hz ≈16.6ms）より十分長く、かつ #667 が導入した
 * `MAX_PLAUSIBLE_FRAME_MS`（250ms・アイドルギャップとして無視するしきい値）より十分短い間隔で
 * 複数回ディスパッチする。on-demand フレームループ（ADR-0126）は各イベントごとに 1 枚だけ
 * `render()` を起こすので、イベント間隔がそのまま連続フレームの dt になり、「アイドル明け」ではなく
 * 確実に「予算超過」と判定される。
 *
 * `?renderer=tiny-skia` は CPU 2D canvas backend で headless Chromium・WebGPU 無しでも動く
 * （`?renderer=vello` は WebGPU が要るためヘッドレスでは使えないことが多い）。canvas 自身の
 * 2D バッファを `getImageData` で直接読み、スクリーンショットでは捉えられない 1 フレームだけの
 * 空白も rAF ごとに記録して見逃さない。
 */
test.describe('Canvas render-scale change does not blank the canvas (ADR-0129, #666/#667/#669)', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => {
      // rAF をラップし、実 render() 呼び出し（＝そのフレームの present）の直後に、canvas の
      // 左上隅ピクセルの alpha を毎フレーム記録する。背景色は必ず canvas 全面を不透明に塗るので、
      // alpha===0 は「このフレームで canvas がクリアされたまま何も描かれなかった」ことを意味する
      // （`canvas.width`/`height` への代入は仕様上バッファを透明にクリアする）。
      const state = window as unknown as { __alphaSamples: number[] };
      state.__alphaSamples = [];
      const originalRAF = window.requestAnimationFrame.bind(window);
      window.requestAnimationFrame = (cb: FrameRequestCallback): number =>
        originalRAF((t) => {
          cb(t);
          const canvas = document.getElementById('canvas-stage') as HTMLCanvasElement | null;
          if (!canvas || canvas.width === 0 || canvas.height === 0) return;
          try {
            const ctx = canvas.getContext('2d');
            if (!ctx) return;
            const alpha = ctx.getImageData(0, 0, 1, 1).data[3];
            state.__alphaSamples.push(alpha);
          } catch {
            // tiny-skia 以外（vello の WebGPU 等）では 2D コンテキストを取得できないことがある。
            // このテストは tiny-skia 固定なので無視してよい。
          }
        });
      window.localStorage.clear();
    });
    // tiny-skia CPU backend: WebGPU の無いヘッドレスでも Canvas モードに入れる。
    await page.goto('/?renderer=tiny-skia');
  });

  test('予算超過フレームが連続してスケールが変わっても、canvas に空白フレームを挟まない', async ({
    page,
  }) => {
    test.setTimeout(60_000);

    const editContextSupported = await page.evaluate(
      () => typeof (globalThis as { EditContext?: unknown }).EditContext !== 'undefined',
    );
    test.skip(!editContextSupported, 'EditContext 非対応ブラウザ（Canvas モードに入れない）');

    const canvas = page.locator('#canvas-stage');
    await expect(canvas).toBeVisible();

    const box = await canvas.boundingBox();
    expect(box, 'canvas bounding box').not.toBeNull();
    if (!box) return;

    // 最初の描画（alpha>0）が済むのを待つ。
    await expect
      .poll(() =>
        page.evaluate(
          () => (window as unknown as { __alphaSamples: number[] }).__alphaSamples.at(-1) ?? 0,
        ),
      )
      .toBeGreaterThan(0);

    // seed タスク一覧（scroll-view）の上へ移動し、wheel を budget を大きく超える間隔で連続
    // ディスパッチする。PRESSURE_FRAMES_TO_DEGRADE(=3) を確実に超える回数を送り、劣化
    // （render_scale 変更）を少なくとも 1 回誘発する。
    await page.mouse.move(box.x + box.width / 2, box.y + box.height * 0.6);
    for (let i = 0; i < 12; i++) {
      await page.mouse.wheel(0, 40);
      await page.waitForTimeout(80);
    }
    // 劣化フレームの apply_render_scale（buffer resize）が実際に走り切るのを少し待つ。
    await page.waitForTimeout(300);

    const samples = await page.evaluate(
      () => (window as unknown as { __alphaSamples: number[] }).__alphaSamples,
    );

    // 最初に描画済み（alpha>0）になった後は、一度も alpha===0（空白フレーム）に戻らないこと。
    const firstPaintedIndex = samples.findIndex((a) => a > 0);
    expect(firstPaintedIndex, 'canvas should have painted at least once').toBeGreaterThanOrEqual(0);
    const afterFirstPaint = samples.slice(firstPaintedIndex);
    const blankFrameIndex = afterFirstPaint.findIndex((a) => a === 0);
    expect(
      blankFrameIndex,
      `canvas went blank (alpha=0) at sample #${blankFrameIndex} after first paint — ` +
        `samples: ${afterFirstPaint.join(',')}`,
    ).toBe(-1);
  });
});
