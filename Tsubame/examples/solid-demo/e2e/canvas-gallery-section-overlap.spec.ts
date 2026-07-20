import { expect, test } from '@playwright/test';

/**
 * 回帰: CSS Gallery（Canvas モード）で「Motion」セクション見出しが直前の
 * 「defaultColor / …」カード（caption: inherited text defaults）に重なるバグ。
 *
 * 原因は vendored taffy の flexbox `ComputeSize`（min/max クランプ後の cross size と
 * クランプ前に計測した main size の自己矛盾ペアがノードキャッシュ経由で再利用され、
 * wrap 行の高さがタイトル 1 行分に固定される）。ネイティブの最小シームは
 * `Hayate/crates/core/tests/flex_wrap_minmax_clamp_reflow.rs`、ギャラリー同型は
 * `gallery_wrap_section_overlap.rs`。この spec は実ブラウザ + WASM + Tsubame 配線まで
 * 通した防衛線。
 *
 * canvas は DOM から黒箱で、a11y ミラーは button 系しか投影しないため、アサートは
 * main.tsx が公開する debug seam `window.__hayateRaw`（Hayate レイアウト正本）で行う。
 * tiny-skia CPU backend（`?renderer=tiny-skia`）なら WebGPU の無いヘッドレス Chromium
 * でも Canvas モードに入れる（prior art: canvas-a11y-mirror.spec.ts）。
 */

/** main.tsx の debug seam。テストが使う照会面だけを型付けする。 */
interface RawHayateProbe {
  element_subtree_ids(id: number): ArrayLike<number>;
  element_get_text(id: number): string;
  element_get_bounds(id: number): ArrayLike<number>;
}

test.describe('Canvas gallery — セクション見出しと前カードの重なり回帰', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => window.localStorage.clear());
    // WASM ビルドが無い環境では canvas モードに入れず #canvas-stage が hidden のまま。
    // その場合はこの spec の対象外としてスキップする（既存 canvas 系 spec と同じ方針）。
    await page.goto('/?renderer=tiny-skia&page=gallery');
  });

  test('Motion 見出しはレイアウト正本上で直前カードの下に置かれる', async ({ page }) => {
    test.setTimeout(90_000);

    const canvasEntered = await page
      .waitForFunction(
        () => (window as unknown as { __hayateRaw?: unknown }).__hayateRaw != null,
        undefined,
        { timeout: 20_000 },
      )
      .then(() => true)
      .catch(() => false);
    test.skip(!canvasEntered, 'Canvas モードに入れない環境（WASM 未ビルド / EditContext 非対応）');

    // 初期レイアウト＋（あれば）フォント到着後の再レイアウトまで静定を待つ。
    await page.waitForTimeout(2_500);

    const r = await page.evaluate(() => {
      const raw = (window as unknown as { __hayateRaw: RawHayateProbe }).__hayateRaw;
      const all = Array.from(raw.element_subtree_ids(1) ?? []);
      const findText = (needle: string): number | null => {
        for (const id of all) {
          let t = '';
          try {
            t = raw.element_get_text(id);
          } catch {
            /* text を持たない要素 */
          }
          if (t && t.trim().startsWith(needle)) return id;
        }
        return null;
      };
      // target を包む最小の box 持ち要素（caption→カード view、Motion→見出し行）。
      const minAnc = (target: number, minSize: number) => {
        let best: { id: number; size: number } | null = null;
        for (const id of all) {
          if (id === target) continue;
          let sub: number[];
          try {
            sub = Array.from(raw.element_subtree_ids(id));
          } catch {
            continue;
          }
          if (sub.length <= minSize) continue;
          if (sub.includes(target) && (!best || sub.length < best.size)) {
            best = { id, size: sub.length };
          }
        }
        if (!best) return null;
        const b = Array.from(raw.element_get_bounds(best.id));
        return { y: b[1], bottom: b[1] + b[3] };
      };
      const captionId = findText('inherited text defaults');
      const motionId = findText('Motion');
      if (captionId == null || motionId == null) return { error: 'targets not found' as const };
      return { card: minAnc(captionId, 4), motionRow: minAnc(motionId, 2) };
    });

    expect(r.error, 'gallery targets must exist in the layout tree').toBeUndefined();
    if ('error' in r && r.error) return;
    expect(r.card, 'caption card bounds').not.toBeNull();
    expect(r.motionRow, 'Motion heading row bounds').not.toBeNull();
    if (!r.card || !r.motionRow) return;

    // 症状そのもの: Motion 見出し行（次セクション）が前カードに食い込まないこと。
    expect(
      r.motionRow.y,
      `Motion heading top (${r.motionRow.y}) must not overlap previous card bottom (${r.card.bottom})`,
    ).toBeGreaterThanOrEqual(r.card.bottom);
  });
});
