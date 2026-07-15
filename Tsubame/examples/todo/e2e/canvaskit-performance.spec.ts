import { expect, test, type Page, type TestInfo } from '@playwright/test';

const CANVASKIT_PERFORMANCE_BUDGET = {
  static: { fullSceneReplays: 1, allocations: 0 },
  textEditing: { fullSceneReplays: 10, allocations: 12 },
  scroll: { fullSceneReplays: 8, allocations: 12 },
  animation: { fullSceneReplays: 30, allocations: 12 },
} as const;

// Opt-in improvement target. The current implementation intentionally trips at least the editing
// allocation or scroll replay assertion, making the harness red-capable without destabilizing CI.
const CANVASKIT_STRICT_RED_BUDGET = {
  static: { fullSceneReplays: 0, allocations: 0 },
  textEditing: { fullSceneReplays: 4, allocations: 0 },
  scroll: { fullSceneReplays: 4, allocations: 0 },
  animation: { fullSceneReplays: 24, allocations: 0 },
} as const;
const STRICT_RED_MODE = process.env.CANVASKIT_PERF_STRICT === '1';

interface CanvasKitPerformanceSnapshot {
  replayCount: number;
  fullSceneReplayCount: number;
  commandPayloadBytes: number;
  paintAllocationCount: number;
  fontAllocationCount: number;
  scratchAllocationCount: number;
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
    metrics.paintAllocationCount + metrics.fontAllocationCount + metrics.scratchAllocationCount;
  expect(metrics.fullSceneReplayCount, `${scenario}: full-scene replay budget`).toBeLessThanOrEqual(
    budget.fullSceneReplays,
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
