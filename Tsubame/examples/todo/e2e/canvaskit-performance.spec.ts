import { expect, test, type Page, type TestInfo } from '@playwright/test';

const CANVASKIT_PERFORMANCE_BUDGET = {
  static: { fullSceneReplays: 0, layerReplays: 0, allocations: 0 },
  textEditing: { fullSceneReplays: 0, layerReplays: 10, allocations: 12 },
  scroll: { fullSceneReplays: 0, layerReplays: 18, allocations: 12 },
  animation: { fullSceneReplays: 0, layerReplays: 72, allocations: 12 },
} as const;

// Opt-in improvement target. The current implementation intentionally trips at least the editing
// allocation or scroll replay assertion, making the harness red-capable without destabilizing CI.
const CANVASKIT_STRICT_RED_BUDGET = {
  static: { fullSceneReplays: 0, layerReplays: 0, allocations: 0 },
  textEditing: { fullSceneReplays: 0, layerReplays: 4, allocations: 0 },
  scroll: { fullSceneReplays: 0, layerReplays: 12, allocations: 0 },
  animation: { fullSceneReplays: 0, layerReplays: 64, allocations: 0 },
} as const;
const STRICT_RED_MODE = process.env.CANVASKIT_PERF_STRICT === '1';

interface CanvasKitPerformanceSnapshot {
  replayCount: number;
  fullSceneReplayCount: number;
  layerReplayCount: number;
  compositeFrameCount: number;
  compositeOnlyFrameCount: number;
  commandPayloadBytes: number;
  commandPayloadAllocationCount: number;
  paintAllocationCount: number;
  fontAllocationCount: number;
  scratchAllocationCount: number;
  commandDecodeAllocationCount: number;
  frameTimeMs: number[];
  webgl: { version: string; renderer: string; software: boolean };
}

type Scenario = keyof typeof CANVASKIT_PERFORMANCE_BUDGET;

async function resetPerformance(page: Page): Promise<void> {
  await page.evaluate(() => {
    const canvas = document.querySelector('#canvas-stage') as HTMLCanvasElement;
    const bridge = (globalThis as unknown as {
      __hayateCanvasKitBridge: { resetPerformance(canvas: HTMLCanvasElement): void };
    }).__hayateCanvasKitBridge;
    bridge.resetPerformance(canvas);
  });
}

async function snapshot(page: Page): Promise<CanvasKitPerformanceSnapshot> {
  return page.evaluate(() => {
    const canvas = document.querySelector('#canvas-stage') as HTMLCanvasElement;
    const bridge = (globalThis as unknown as {
      __hayateCanvasKitBridge: {
        performanceSnapshot(canvas: HTMLCanvasElement): CanvasKitPerformanceSnapshot;
      };
    }).__hayateCanvasKitBridge;
    return bridge.performanceSnapshot(canvas);
  });
}

async function settle(page: Page, frames = 2): Promise<void> {
  await page.evaluate(async (count) => {
    for (let index = 0; index < count; index += 1) {
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    }
  }, frames);
}

async function dispatchWheelWithoutPointerMove(page: Page, deltaY: number): Promise<void> {
  await page.evaluate((wheelDeltaY) => {
    const canvas = document.querySelector('#canvas-stage') as HTMLCanvasElement;
    canvas.dispatchEvent(new WheelEvent('wheel', {
      deltaY: wheelDeltaY,
      clientX: 1000,
      clientY: 500,
      bubbles: true,
      cancelable: true,
    }));
  }, deltaY);
}

async function attachReport(
  scenario: Scenario,
  metrics: CanvasKitPerformanceSnapshot,
  testInfo: TestInfo,
): Promise<void> {
  const sorted = [...metrics.frameTimeMs].sort((a, b) => a - b);
  const percentile = (ratio: number) => sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * ratio))] ?? 0;
  const report = {
    scenario,
    ...metrics,
    frameTimeP50Ms: percentile(0.5),
    frameTimeP95Ms: percentile(0.95),
  };
  await testInfo.attach(`canvaskit-${scenario}.json`, {
    body: JSON.stringify(report, null, 2),
    contentType: 'application/json',
  });
  console.log(`[CanvasKit perf] ${JSON.stringify(report)}`);
}

function assertDeterministicBudget(scenario: Scenario, metrics: CanvasKitPerformanceSnapshot): void {
  const budget = STRICT_RED_MODE
    ? CANVASKIT_STRICT_RED_BUDGET[scenario]
    : CANVASKIT_PERFORMANCE_BUDGET[scenario];
  const allocations =
    metrics.paintAllocationCount +
    metrics.fontAllocationCount +
    metrics.scratchAllocationCount +
    metrics.commandDecodeAllocationCount;
  expect(metrics.fullSceneReplayCount, `${scenario}: full-scene replay budget`).toBeLessThanOrEqual(
    budget.fullSceneReplays,
  );
  expect(metrics.layerReplayCount, `${scenario}: dirty-layer replay budget`).toBeLessThanOrEqual(
    budget.layerReplays,
  );
  expect(allocations, `${scenario}: hot-path allocation budget`).toBeLessThanOrEqual(
    budget.allocations,
  );
  expect(metrics.commandPayloadBytes).toBeGreaterThanOrEqual(0);
}

test.describe('CanvasKit real Chromium performance feedback loop', () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(() => window.localStorage.clear());
    await page.goto('/?renderer=canvaskit');
    await expect(page.locator('#canvas-stage')).toBeVisible();
    await page.waitForFunction(() => {
      const target = globalThis as unknown as { __hayateCanvasKitBridge?: unknown };
      return target.__hayateCanvasKitBridge !== undefined;
    });
    await settle(page, 3);
  });

  test('static shared fixture stays idle', async ({ page }, testInfo) => {
    await resetPerformance(page);
    await settle(page, 4);
    const metrics = await snapshot(page);
    expect(metrics.webgl.version).not.toBe('unavailable');
    expect(metrics.webgl.renderer).not.toBe('unavailable');
    await attachReport('static', metrics, testInfo);
    assertDeterministicBudget('static', metrics);
  });

  test('dirty-layer present matches the single-root CanvasKit output', async ({ page }, testInfo) => {
    await page.goto('/?renderer=canvaskit&layerPresent=0');
    await expect(page.locator('#canvas-stage')).toBeVisible();
    await settle(page, 3);
    const singleRoot = await page.locator('#canvas-stage').screenshot({
      path: testInfo.outputPath('single-root.png'),
    });

    await page.goto('/?renderer=canvaskit&layerPresent=1');
    await expect(page.locator('#canvas-stage')).toBeVisible();
    await settle(page, 3);
    const layered = await page.locator('#canvas-stage').screenshot({
      path: testInfo.outputPath('dirty-layer.png'),
    });

    expect(layered.equals(singleRoot), 'CanvasKit layer order/clip/shadow parity').toBe(true);
  });

  for (const device of [
    { name: 'desktop-dpr1', viewport: { width: 1280, height: 720 }, deviceScaleFactor: 1 },
    { name: 'mobile-dpr3', viewport: { width: 400, height: 720 }, deviceScaleFactor: 3 },
  ] as const) {
    test(`composite-only scroll-first-frame output matches single-root (${device.name})`, async ({
      browser,
    }, testInfo) => {
      const context = await browser.newContext({
        baseURL: testInfo.project.use.baseURL,
        viewport: device.viewport,
        deviceScaleFactor: device.deviceScaleFactor,
        isMobile: device.name.startsWith('mobile'),
        hasTouch: device.name.startsWith('mobile'),
      });

      const capture = async (layerPresent: 0 | 1): Promise<Buffer> => {
        const page = await context.newPage();
        await page.addInitScript(() => window.localStorage.clear());
        await page.goto(`/?renderer=canvaskit&layerPresent=${layerPresent}`);
        await expect(page.locator('#canvas-stage')).toBeVisible();
        await page.waitForFunction(() => {
          const target = globalThis as unknown as { __hayateCanvasKitBridge?: unknown };
          return target.__hayateCanvasKitBridge !== undefined;
        });
        await settle(page, 3);
        // Initial transitions/font resolution must not race the wheel frame. Capture inside the
        // first backend present call after arming so a later kinetic-scroll frame cannot replace
        // the broken transient frame before Playwright takes its screenshot.
        await page.waitForTimeout(400);
        await settle(page, 3);
        await page.evaluate((layered) => {
          const canvas = document.querySelector('#canvas-stage') as HTMLCanvasElement;
          const target = globalThis as unknown as {
            __hayateFirstScrollFrame?: string;
            __hayateCanvasKitBridge: Record<string, (...args: unknown[]) => unknown>;
          };
          const method = layered === 1 ? 'compositeLayers' : 'replay';
          const bridge = target.__hayateCanvasKitBridge;
          const original = bridge[method]!;
          target.__hayateFirstScrollFrame = undefined;
          bridge[method] = function (...args: unknown[]): unknown {
            const result = original.apply(this, args);
            if (args[0] === canvas && target.__hayateFirstScrollFrame === undefined) {
              target.__hayateFirstScrollFrame = canvas.toDataURL('image/png');
              bridge[method] = original;
            }
            return result;
          };
        }, layerPresent);
        await page.mouse.move(device.viewport.width / 2, device.viewport.height * 0.7);
        // 24px stays inside the initial cache band on both fixtures. This must exercise the
        // composite-only path; a larger jump can re-raster and hide a viewport clip baked into
        // the old texture.
        await page.mouse.wheel(0, 24);
        const dataUrl = await page.waitForFunction(
          () => (globalThis as unknown as { __hayateFirstScrollFrame?: string })
            .__hayateFirstScrollFrame,
        ).then((handle) => handle.jsonValue() as Promise<string>);
        const image = Buffer.from(dataUrl.slice(dataUrl.indexOf(',') + 1), 'base64');
        await testInfo.attach(`${device.name}-layer-present-${layerPresent}.png`, {
          body: image,
          contentType: 'image/png',
        });
        await page.close();
        return image;
      };

      const singleRoot = await capture(0);
      const layered = await capture(1);
      expect(
        layered.equals(singleRoot),
        `${device.name}: first rendered frame after scroll must match layerPresent=0`,
      ).toBe(true);
      await context.close();
    });
  }

  test('text editing does not replay or allocate excessively', async ({ page }, testInfo) => {
    const mirror = page.locator('[data-hayate-a11y]');
    await expect(mirror).toHaveCount(1);
    const textbox = mirror.getByRole('textbox');
    await expect(textbox).toHaveCount(1);
    const box = await textbox.boundingBox();
    expect(box).not.toBeNull();
    if (!box) return;
    await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
    await resetPerformance(page);
    await page.keyboard.type('perf');
    await settle(page, 3);
    const metrics = await snapshot(page);
    await attachReport('textEditing', metrics, testInfo);
    assertDeterministicBudget('textEditing', metrics);
  });

  test('scroll does not replay or allocate excessively', async ({ page }, testInfo) => {
    await resetPerformance(page);
    await page.mouse.move(400, 500);
    await page.mouse.wheel(0, 320);
    await settle(page, 3);
    const metrics = await snapshot(page);
    await attachReport('scroll', metrics, testInfo);
    assertDeterministicBudget('scroll', metrics);
  });

  test('in-band scroll reuses CanvasKit layer snapshots', async ({ page }) => {
    // Let initial font resolution and transitions settle before isolating one small wheel input.
    // 24px remains inside the initial 600px overscan band, so content replay would indicate that
    // Core dirty state leaked into CanvasKit instead of taking the composite-only path.
    await page.waitForTimeout(400);
    await settle(page, 3);
    await resetPerformance(page);

    // Dispatch at a coordinate inside the scroll viewport without a preceding pointermove. A real
    // mouse move changes :hover and can start an unrelated transform layer transition.
    await dispatchWheelWithoutPointerMove(page, 24);
    await settle(page, 3);

    const metrics = await snapshot(page);
    expect(metrics.fullSceneReplayCount, 'layer present must remain enabled').toBe(0);
    expect(metrics.layerReplayCount, 'in-band pure scroll must not replay cached content').toBe(0);
    expect(
      metrics.compositeOnlyFrameCount,
      'the scroll offset must still produce a cached-layer composite',
    ).toBeGreaterThan(0);
  });

  test('scrolling beyond the cached band replays only CanvasKit scroll content', async ({ page }) => {
    await page.waitForTimeout(400);
    await settle(page, 3);
    await resetPerformance(page);

    await dispatchWheelWithoutPointerMove(page, 900);
    await settle(page, 3);

    const metrics = await snapshot(page);
    expect(metrics.fullSceneReplayCount, 'overscan refresh must stay on layer present').toBe(0);
    expect(metrics.layerReplayCount, 'only the expired scroll snapshot should replay').toBe(1);
    expect(
      metrics.compositeFrameCount,
      'the refreshed snapshot must be composited',
    ).toBeGreaterThan(0);
  });

  test('theme transition animation stays within its named replay budget', async ({ page }, testInfo) => {
    const mirror = page.locator('[data-hayate-a11y]');
    const themeButton = mirror.getByRole('button', { name: '🌙' });
    await expect(themeButton).toHaveCount(1);
    const box = await themeButton.boundingBox();
    expect(box).not.toBeNull();
    if (!box) return;
    await resetPerformance(page);
    await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
    await page.waitForTimeout(350);
    const metrics = await snapshot(page);
    await attachReport('animation', metrics, testInfo);
    assertDeterministicBudget('animation', metrics);
  });
});
