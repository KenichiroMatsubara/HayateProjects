import { expect, test } from '@playwright/test';

/**
 * 実ブラウザ回帰: Canvas モードで、非編集テキスト（kind=`text`）の上にホバーすると
 * カーソルが I-beam（`text`）になる。
 *
 * 旧実装は core の `resolve_cursor` が「選択可能性 = `selectable` Selection Region
 * ルート」だけを見ていたため、明示 region を持たない素のテキスト段落は既定カーソル
 * （矢印）のままだった。ADR-0105 は「選択可能テキスト = text」を、ADR-0108 は
 * 「element-kind 既定で text/view/scroll-view は既定選択可（opt-out）」を決めている。
 * 修正後は effective `user-select == text` かつ text-bearing な要素で I-beam を返し、
 * `apply_resolved_cursor` が canvas の `style.cursor` に反映する。
 *
 * Canvas モードは DOM ノードを持たない（全要素を canvas に描画）ため、テキストの
 * DOM セレクタは無い。よって canvas をポインタで走査し、`canvas.style.cursor` を
 * 観測する。DOM 経路はブラウザネイティブ選択 + UA `user-select` で元から I-beam に
 * なるので、ここでは Canvas 固有のカーソル配線（ADR-0105 が「未結線」と指摘していた
 * 箇所）を確認する。選択の挙動そのものは core 所有（ADR-0097）で core 単体テスト
 * （`plain_text_selection.rs` / `text_selection.rs`）が厳密に固定している。
 *
 * Canvas モードは EditContext 対応ブラウザ専用 (ADR-0016/0048)。未対応なら
 * アプリは DOM モードへ自動フォールバックするので、その場合はスキップする。
 */
test.describe('Canvas text cursor — I-beam over selectable text', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => window.localStorage.clear());
    // tiny-skia CPU backend: WebGPU の無いヘッドレスでも Canvas モードに入れる。
    await page.goto('/?renderer=tiny-skia');
  });

  test('seed タスク一覧（素のテキスト）の上で cursor が text になる', async ({ page }) => {
    test.setTimeout(60_000);
    const canvas = page.locator('#canvas-stage');

    const editContextSupported = await page.evaluate(
      () => typeof (globalThis as { EditContext?: unknown }).EditContext !== 'undefined',
    );
    test.skip(!editContextSupported, 'EditContext 非対応ブラウザ（DOM モード）');

    await expect(canvas).toBeVisible();
    await page.waitForTimeout(200);

    const box = await canvas.boundingBox();
    expect(box, 'canvas bounding box').not.toBeNull();
    if (!box) return;

    // canvas をグリッド走査し、各点でホバー→ `style.cursor` を集める。素の text
    // ラベルが並ぶコンテンツ帯のどこかで `text`（I-beam）が出れば、修正後の挙動。
    // 修正前は text-input 以外の text は矢印のままなので、入力欄を避けた下半分で
    // `text` が出ることがバグ1の実ブラウザ証明になる。
    // pointermove → core → `apply_resolved_cursor` は pointermove ハンドラ内で
    // 同期的に canvas の `style.cursor` を設定するので、move 直後に読める。
    const cursors = new Set<string>();
    const cols = 5;
    const rows = 6;
    for (let r = 1; r < rows; r++) {
      for (let c = 1; c < cols; c++) {
        const x = box.x + (box.width * c) / cols;
        const y = box.y + (box.height * r) / rows;
        await page.mouse.move(x, y);
        const cursor = await canvas.evaluate((el) => (el as HTMLCanvasElement).style.cursor);
        if (cursor) cursors.add(cursor);
      }
    }

    expect(
      cursors.has('text'),
      `選択可能テキストの上で I-beam（text）が出るべき。観測した cursor 値: ${[...cursors].join(', ')}`,
    ).toBe(true);
  });
});
