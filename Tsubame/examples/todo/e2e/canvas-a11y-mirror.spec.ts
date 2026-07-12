import { expect, test } from '@playwright/test';

/**
 * Accessibility Mirror（ADR-0124）の capstone e2e（#595）。「クラウド実行の AI が Canvas アプリを
 * 照会・駆動・回帰ガードできる」状態をエンドツーエンドで証拠化する縦スライス。
 *
 * Canvas モードの `<canvas>` はアクセシビリティツリーから黒箱だが、`@torimi/hayate-host` の
 * `attachAccessibilityMirror`（#591-#594）が `poll_accessibility()` の AccessKit `TreeUpdate` を
 * `<canvas>` 兄弟の不可視 ARIA DOM（`data-hayate-a11y`）へ投影する。これにより Playwright の
 * `getByRole` / `toMatchAriaSnapshot` で **照会** でき、bounds から得た座標で **駆動** でき、
 * focus 反映で **状態変化を再アサート** できる。
 *
 * ミラーは `opacity:0` ＋ `pointer-events:none`。座標クリックはミラーを素通りして下の `<canvas>`
 * に届く（ミラーは横取りしない）。`getByRole().click()` の直接駆動は pointer-events:none のため不可で、
 * v1 は意味的特定 → 座標ホップで駆動する（ADR-0124）。
 *
 * 役割は **element-kind** から導かれる: `text-input`→`textbox`、`button`→`button`、`view`→generic、
 * `text`→label。アプリ側の明示 `role`/`aria-label` 注入（list/listitem 等）は Tsubame→Hayate の
 * wire に未配線で、本スライスのスコープ外（読み取り専用ミラーは kind 由来の役割を投影する）。
 *
 * tiny-skia CPU backend（`?renderer=tiny-skia`）なら WebGPU の無いヘッドレス Chromium でも Canvas
 * モードに入れる。EditContext 非対応ブラウザは DOM モードへ自動フォールバックするので skip する
 * （prior art: `canvas-text-cursor.spec.ts` / `torimi-host.spec.ts`）。
 */
test.describe('Canvas a11y mirror — AI queries, drives, and re-asserts the canvas app (ADR-0124)', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => window.localStorage.clear());
    // tiny-skia CPU backend: WebGPU の無いヘッドレスでも Canvas モードに入れる。
    await page.goto('/?renderer=tiny-skia');
  });

  test('getByRole で照会 → 矩形座標で駆動 → focus 反映を再アサートする', async ({ page }) => {
    test.setTimeout(60_000);

    const editContextSupported = await page.evaluate(
      () => typeof (globalThis as { EditContext?: unknown }).EditContext !== 'undefined',
    );
    test.skip(!editContextSupported, 'EditContext 非対応ブラウザ（Canvas モードに入れない）');

    const canvas = page.locator('#canvas-stage');
    await expect(canvas).toBeVisible();

    // ミラーが canvas 兄弟に建ち、最初の投影が走るまで待つ（rAF poll → ARIA DOM）。
    const mirror = page.locator('[data-hayate-a11y]');
    await expect(mirror).toHaveCount(1);

    // ── 照会（query）: role/name で対象を意味的に特定する（#592）。
    // seed UI は追加フォーム（text-input → textbox）と「追加」ボタン（button）を持つ。
    const textbox = mirror.getByRole('textbox').first();
    await expect(textbox).toBeVisible();
    const addButton = mirror.getByRole('button', { name: '追加' });
    await expect(addButton).toBeVisible();

    // UI 構造の baseline: 「追加」ボタンが accessible name を伴って投影されていること。
    // aria-snapshot が安定して緑になる = 役割と名前計算が回帰なく効いている証拠。
    await expect(addButton).toMatchAriaSnapshot('- button "追加"');

    // ── 駆動（drive）: 意味的に特定した textbox の矩形から座標を得て、canvas をクリックする（#593）。
    // ミラーは pointer-events:none なので、座標は下の <canvas> に届き入力欄に focus が入る
    //（ミラーがクリックを横取りしない回帰ガードも兼ねる）。
    const box = await textbox.boundingBox();
    expect(box, 'textbox bounding box').not.toBeNull();
    if (!box) return;
    await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);

    // ── 再アサート（re-assert）: focus 変化がミラーに反映される（#594）。
    // root の aria-activedescendant が、いま座標クリックした textbox の id を指す。
    const textboxId = await textbox.getAttribute('id');
    expect(textboxId, 'mirror textbox id').toBeTruthy();
    await expect(mirror).toHaveAttribute('aria-activedescendant', textboxId!);
  });

  test('deep AppBar タブの矩形座標クリックが実描画位置に当たり画面遷移する（#756: 入れ子座標の加算回帰ガード）', async ({
    page,
  }) => {
    test.setTimeout(60_000);
    const editContextSupported = await page.evaluate(
      () => typeof (globalThis as { EditContext?: unknown }).EditContext !== 'undefined',
    );
    test.skip(!editContextSupported, 'EditContext 非対応ブラウザ（Canvas モードに入れない）');

    const canvas = page.locator('#canvas-stage');
    await expect(canvas).toBeVisible();
    const mirror = page.locator('[data-hayate-a11y]');
    await expect(mirror).toHaveCount(1);

    // #756 の回帰ガード: AppBar「CSS Gallery」タブは a11y ツリーの深い入れ子ノード。ミラーが各ノードを
    // 絶対座標のまま `position:absolute` で入れ子配置すると、親のオフセットが加算され boundingBox が
    // 実描画位置の右下へ大きくずれる（実測: 実描画 x0≈663 が 1251 に化ける）。座標クリックは実 canvas の
    // 何もない所に落ち、タブは反応しない。ミラー矩形の中心をクリックして **実際に画面が切り替わる**
    // ことを、CSS Gallery ページ固有の投影テキスト（セクション見出し "Visual"）の出現で証拠化する。
    const galleryTab = mirror.getByRole('button', { name: 'CSS Gallery' });
    await expect(galleryTab).toBeVisible();

    // 遷移前は Tasks ページなので gallery 見出しは無い。
    await expect(mirror.getByText('Visual', { exact: true })).toHaveCount(0);

    const box = await galleryTab.boundingBox();
    expect(box, 'CSS Gallery tab bounding box').not.toBeNull();
    if (!box) return;
    await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);

    // クリックが実タブに当たっていれば CSS Gallery ページへ遷移し、セクション見出しがミラーに現れる。
    // 座標が加算でずれていれば canvas の余白に落ち、遷移せずこの assert が落ちる（回帰を捕捉）。
    await expect(mirror.getByText('Visual', { exact: true }).first()).toBeVisible();
  });
});
