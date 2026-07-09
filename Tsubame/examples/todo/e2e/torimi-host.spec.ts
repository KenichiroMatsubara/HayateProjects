import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, test } from '@playwright/test';

/**
 * Torimi の e2e（ADR-0001）。最小 dev server が HTTP 配信する単一 App Bundle を、Torimi
 * ホストページ（host.html）が fetch → eval し、`createHayateWebHost` で canvas 上に host bootstrap
 * を確立してバンドルの mount に渡す。さらに full reload ループ（ソース編集 → WS reload → 再 mount）を
 * 本物の Chromium で端から端まで検証する。
 *
 * ホスト側は framework / renderer-hayate を持たず、それらは eval するバンドルが持ち込む。
 *
 * 両テストは同じ dev-server / host.html を共有し、reload テストのソース編集は接続中の全 host へ
 * 配信される。テスト間で host ページが同時に開かないよう、このファイルは **serial** で走らせる。
 */
test.describe.configure({ mode: 'serial' });

const TORIMI_DEV_PORT = Number(process.env.TORIMI_DEV_PORT ?? 5181);
const DEV_SERVER_URL = `http://localhost:${TORIMI_DEV_PORT}`;

/** full reload e2e が編集する App Bundle のソース。コメント追記で再ビルドを誘発する。 */
const RELOAD_EDIT_TARGET = fileURLToPath(new URL('../src/main.bundle.tsx', import.meta.url));

test.describe('Torimi host — renders the HTTP-served Tsubame bundle', () => {
  test.beforeEach(async ({ page }) => {
    // localStorage の持ち越しを避け、seed todo から始める。
    await page.addInitScript(() => window.localStorage.clear());
  });

  test('ホストページを開くと todo が canvas に描画される', async ({ page }) => {
    test.setTimeout(60_000);
    await page.goto(`/host.html?dev=${encodeURIComponent(DEV_SERVER_URL)}`);

    // fetch → eval → createHayateWebHost → mount が端から端まで貫けたこと。
    await expect(page.locator('html')).toHaveAttribute('data-torimi-status', 'mounted', {
      timeout: 30_000,
    });

    const canvas = page.locator('#torimi-canvas');
    await expect(canvas).toBeVisible();

    // surface 上にレンダラが初期化され backing store が確保されたこと。
    await expect
      .poll(async () => canvas.evaluate((el) => (el as HTMLCanvasElement).width))
      .toBeGreaterThan(0);

    // 描画証明：tiny-skia Canvas は DOM テキストを持たないので、seed todo（選択可能テキスト）の
    // 上で I-beam（text）カーソルが出ることを以て「todo が描画された」とする
    // （canvas-text-cursor.spec と同じ手法 / ADR-0105）。EditContext 非対応なら Canvas モードに
    // 入れないため skip。
    const editContextSupported = await page.evaluate(
      () => typeof (globalThis as { EditContext?: unknown }).EditContext !== 'undefined',
    );
    test.skip(!editContextSupported, 'EditContext 非対応ブラウザ（Canvas モードに入れない）');

    await page.waitForTimeout(300);
    const box = await canvas.boundingBox();
    expect(box, 'canvas bounding box').not.toBeNull();
    if (!box) return;

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
      `seed todo（選択可能テキスト）の上で I-beam（text）が出るべき。観測した cursor: ${[...cursors].join(', ')}`,
    ).toBe(true);
  });
});

test.describe('Torimi host — full reload on source change', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => window.localStorage.clear());
  });

  test('examples/todo のソースを編集するとホストが手動リロード無しで再 mount する', async ({
    page,
  }) => {
    test.setTimeout(120_000);
    await page.goto(`/host.html?dev=${encodeURIComponent(DEV_SERVER_URL)}`);

    // 初回 boot 完了（mount count = 1）まで待つ。
    await expect(page.locator('html')).toHaveAttribute('data-torimi-status', 'mounted', {
      timeout: 30_000,
    });
    const baseline = await readMountCount(page);
    expect(baseline).toBeGreaterThanOrEqual(1);

    const original = readFileSync(RELOAD_EDIT_TARGET, 'utf8');
    try {
      // ソース編集 → `vite build --watch` が再ビルド → dev-server が bundle 変更を検知 →
      // WS で `reload` → ホストが再 fetch + 新しい canvas で再 mount（手動リロード無し）。
      writeFileSync(RELOAD_EDIT_TARGET, `${original}\n// torimi full-reload e2e touch\n`);

      // mount count が baseline を超える = full reload ループが端から端まで貫けた。
      await expect
        .poll(async () => readMountCount(page), { timeout: 90_000 })
        .toBeGreaterThan(baseline);
    } finally {
      // ソースを必ず元に戻す（作業ツリーを汚さない）。
      writeFileSync(RELOAD_EDIT_TARGET, original);
    }
  });
});

test.describe('Torimi host — protocol version mismatch', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => window.localStorage.clear());
  });

  test('ホスト版数がバンドルと食い違うと明示エラーになり mount もクラッシュもしない', async ({
    page,
  }) => {
    test.setTimeout(60_000);

    // ページの致命的エラー（謎クラッシュ）が起きていないことも併せて検証する。
    const pageErrors: string[] = [];
    page.on('pageerror', (err) => pageErrors.push(String(err)));

    // `?protocolVersion=999` でホスト（decoder）版数を上書きし、バンドル（encoder）の版数と
    // 食い違わせる。ホストは fetch → eval 後に突き合わせ、不一致を検知する。
    await page.goto(`/host.html?dev=${encodeURIComponent(DEV_SERVER_URL)}&protocolVersion=999`);

    // 明示エラー UI に落ちる（mount もクラッシュもしない）。
    await expect(page.locator('html')).toHaveAttribute(
      'data-torimi-status',
      'protocol-mismatch',
      { timeout: 30_000 },
    );

    // 「このホストは protocol vX、バンドルは vY」を画面に出す。
    const panel = page.locator('#torimi-error');
    await expect(panel).toBeVisible();
    await expect(panel).toContainText('999');
    await expect(panel).toContainText('protocol');

    // mount には到達していない（mount count 未設定 = 一度も mount していない）。
    expect(await readMountCount(page)).toBe(0);

    // 不一致は明示エラーで止め、未捕捉例外（クラッシュ）にしていない。
    expect(pageErrors, `unexpected page errors:\n${pageErrors.join('\n')}`).toEqual([]);
  });
});

/** host ページが data 属性に出している mount 回数を読む（未設定なら 0）。 */
async function readMountCount(page: import('@playwright/test').Page): Promise<number> {
  const value = await page.locator('html').getAttribute('data-torimi-mount-count');
  return value ? Number(value) : 0;
}
