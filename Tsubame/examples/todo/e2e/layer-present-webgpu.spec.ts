import { expect, test, type Page } from '@playwright/test';

/**
 * #697: 実 Chromium（`playwright.config.layer-present.ts`、通常ブランチ限定）で
 * `navigator.gpu.requestAdapter()` の成否を明示的に記録し（取れなければ理由付きで `test.skip`）、
 * 取れた場合は layer-present OFF（`Hayate/wasm-pkgs/pkg`）/ ON（`pkg-layer-present`、
 * `pnpm --filter hayate build:layer-present`）の2ビルドで AddForm.tsx の優先度セグメント
 * （`seg()`、#680/#692 実物パターン）をクリックし、canvas 出力が画素単位で一致することと
 * 実ブラウザでのクリック→フレームのレイテンシ p50/p95 を記録する。
 */

const OFF_PORT = Number(process.env.E2E_LAYER_PRESENT_OFF_PORT ?? 5185);
const ON_PORT = Number(process.env.E2E_LAYER_PRESENT_ON_PORT ?? 5186);
const OFF_URL = `http://localhost:${OFF_PORT}/?renderer=vello`;
const ON_URL = `http://localhost:${ON_PORT}/?renderer=vello`;
const SCENE_RENDERER_LOG_TIMEOUT = 15_000;

/** navigator.gpu.requestAdapter() が実アダプタを返すか。 */
async function probeAdapter(page: Page): Promise<boolean> {
  return page.evaluate(async () => {
    const gpu = (navigator as unknown as { gpu?: { requestAdapter(): Promise<unknown> } }).gpu;
    if (!gpu) return false;
    try {
      return (await gpu.requestAdapter()) != null;
    } catch {
      return false;
    }
  });
}

/**
 * 指定 URL へ遷移し、console ログから `selected scene renderer: vello`（tiny-skia フォールバック
 * していないこと）を確認する。ログ収集は goto 前に仕込む必要がある（初期化ログを取りこぼさない）。
 */
async function gotoAndAssertVello(page: Page, url: string): Promise<void> {
  const logs: string[] = [];
  page.on('console', (msg) => logs.push(msg.text()));
  await page.addInitScript(() => window.localStorage.clear());
  await page.goto(url);
  await expect
    .poll(() => logs.some((l) => l.includes('selected scene renderer')), {
      timeout: SCENE_RENDERER_LOG_TIMEOUT,
    })
    .toBe(true);
  expect(
    logs.some((l) => l.includes('selected scene renderer: vello')),
    `${url} fell back away from vello — logs:\n${logs.join('\n')}`,
  ).toBe(true);
}

test.describe('WebGPU adapter probe (#697)', () => {
  test('navigator.gpu.requestAdapter() の成否を明示的に記録する', async ({ page }) => {
    test.setTimeout(60_000);

    await page.addInitScript(() => window.localStorage.clear());
    await page.goto(OFF_URL);

    const adapterResult = await page.evaluate(async () => {
      const gpu = (navigator as unknown as { gpu?: { requestAdapter(): Promise<unknown> } }).gpu;
      if (!gpu) return { hasGpuObject: false, adapterObtained: false };
      try {
        const adapter = await gpu.requestAdapter();
        return { hasGpuObject: true, adapterObtained: adapter != null };
      } catch {
        return { hasGpuObject: true, adapterObtained: false };
      }
    });

    console.log(
      `[#697] navigator.gpu present: ${adapterResult.hasGpuObject}, ` +
        `requestAdapter() succeeded: ${adapterResult.adapterObtained}`,
    );

    test.skip(
      !adapterResult.adapterObtained,
      `WebGPU adapter unavailable in this Chromium/GPU environment ` +
        `(navigator.gpu present: ${adapterResult.hasGpuObject}) even with ` +
        `--enable-unsafe-webgpu --ignore-gpu-blocklist --use-angle=vulkan. See e2e/README.md (#697).`,
    );

    expect(adapterResult.adapterObtained).toBe(true);
  });

  test('layer-present OFF/ON 両ビルドとも tiny-skia へフォールバックせず vello を選ぶ', async ({
    page,
    browser,
  }) => {
    test.setTimeout(60_000);

    const adapterObtained = await (async () => {
      await page.goto(OFF_URL);
      return probeAdapter(page);
    })();
    test.skip(!adapterObtained, 'WebGPU adapter unavailable — see e2e/README.md (#697).');

    await gotoAndAssertVello(page, OFF_URL);

    const onContext = await browser.newContext();
    const onPage = await onContext.newPage();
    await gotoAndAssertVello(onPage, ON_URL);
    await onContext.close();
  });

  test('優先度セグメントのトグルが layer-present OFF/ON で画素単位で一致し、フレーム遅延を記録する', async ({
    page,
    browser,
  }) => {
    test.setTimeout(60_000);

    const adapterObtained = await (async () => {
      await page.goto(OFF_URL);
      return probeAdapter(page);
    })();
    test.skip(!adapterObtained, 'WebGPU adapter unavailable — see e2e/README.md (#697).');

    await gotoAndAssertVello(page, OFF_URL);
    const onContext = await browser.newContext();
    const onPage = await onContext.newPage();
    await gotoAndAssertVello(onPage, ON_URL);

    // ── 画素パリティ: 優先度セグメント（AddForm.tsx の seg()、#680/#692 実物パターン）をクリックし、
    // canvas 出力（`toDataURL`、WebGPU canvas でも実装依存なく効く）が OFF/ON で一致することを見る。
    // 既定の draftPrio は 2=中 なので、実際に active 状態が変わる '高' へトグルして遷移を発火させる。
    const offImage = await clickPriorityAndReadCanvas(page, '高');
    const onImage = await clickPriorityAndReadCanvas(onPage, '高');
    expect(onImage.length, 'canvas output should be non-empty').toBeGreaterThan(100);
    expect(
      onImage === offImage,
      'layer-present ON canvas output differs from OFF after the same priority toggle',
    ).toBe(true);

    // ── フレーム遅延: セグメントを連続クリックし、クリック→次フレームのレイテンシ p50/p95 を記録する。
    // 環境ノイズ（CDP round-trip 込み）があるため OFF/ON の優劣は assert せず、実測値を成果物として残す。
    const offLatencies = await measureClickToFrameLatencies(page, 12);
    const onLatencies = await measureClickToFrameLatencies(onPage, 12);
    const offStats = percentiles(offLatencies);
    const onStats = percentiles(onLatencies);
    console.log(
      `[#697] click→frame latency (ms) — OFF: p50=${offStats.p50.toFixed(2)} p95=${offStats.p95.toFixed(2)}; ` +
        `ON: p50=${onStats.p50.toFixed(2)} p95=${onStats.p95.toFixed(2)}`,
    );
    expect(Number.isFinite(offStats.p50) && Number.isFinite(onStats.p50)).toBe(true);

    await onContext.close();
  });
});

/** 指定ラベルの優先度セグメントを a11y mirror（ADR-0124）経由で特定し、クリックして canvas を読む。 */
async function clickPriorityAndReadCanvas(page: Page, label: string): Promise<string> {
  const mirror = page.locator('[data-hayate-a11y]');
  await expect(mirror).toHaveCount(1);
  const seg = mirror.getByRole('button', { name: label });
  await expect(seg).toBeVisible();
  const box = await seg.boundingBox();
  if (!box) throw new Error(`segment button "${label}" has no bounding box`);
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  // トグルの遷移・再描画が終わるのを少し待つ（EASE アニメーション込み）。
  await page.waitForTimeout(300);
  return page.evaluate(
    () => (document.getElementById('canvas-stage') as HTMLCanvasElement).toDataURL(),
  );
}

/** 優先度セグメントを順にクリックし、クリック→次フレームのレイテンシ（ms）を集める。 */
async function measureClickToFrameLatencies(page: Page, count: number): Promise<number[]> {
  const labels = ['高', '中', '低'];
  const mirror = page.locator('[data-hayate-a11y]');
  const deltas: number[] = [];
  for (let i = 0; i < count; i++) {
    const label = labels[i % labels.length];
    const seg = mirror.getByRole('button', { name: label });
    const box = await seg.boundingBox();
    if (!box) throw new Error(`segment button "${label}" has no bounding box`);
    const framePromise = page.evaluate(
      () => new Promise<number>((resolve) => requestAnimationFrame(() => resolve(performance.now()))),
    );
    const t0 = Date.now();
    await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
    await framePromise;
    deltas.push(Date.now() - t0);
  }
  return deltas;
}

function percentiles(samples: number[]): { p50: number; p95: number } {
  const sorted = [...samples].sort((a, b) => a - b);
  const at = (q: number) => sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * q))];
  return { p50: at(0.5), p95: at(0.95) };
}
