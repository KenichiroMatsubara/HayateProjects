import { expect, test, type Page } from '@playwright/test';

/**
 * Named acceptance workload (ADR-0156). Debug builds and per-frame diagnostics may help diagnose a
 * failure, but these values alone are not adoption evidence; correctness comes from the shared
 * pipeline counters plus the final composited pixels.
 */
const CANVAS_WORKER_TOUCH_SCROLL_WORKLOAD = {
  viewport: { width: 320, height: 568 },
  deviceScaleFactor: 3,
  warmupRuns: 1,
  sampleRuns: 5,
  movesPerRun: 50,
  frameBudgetMs60Hz: 16.67,
  settleTimeoutMs: 10_000,
} as const;

test.use({
  viewport: CANVAS_WORKER_TOUCH_SCROLL_WORKLOAD.viewport,
  deviceScaleFactor: CANVAS_WORKER_TOUCH_SCROLL_WORKLOAD.deviceScaleFactor,
  hasTouch: true,
  isMobile: true,
});

interface PipelineObservation {
  accepted: number;
  coalesced: number;
  dropped: number;
  active: boolean;
  pending: number;
  failure: boolean;
}

async function pipelineObservation(page: Page): Promise<PipelineObservation> {
  return page.evaluate(async () => {
    const host = (
      window as unknown as {
        __hayateHost?: {
          pipelineObservation(): Promise<PipelineObservation>;
        };
      }
    ).__hayateHost;
    if (!host) throw new Error('standard Canvas Worker host is not mounted');
    return host.pipelineObservation();
  });
}

async function dispatchTouchScroll(page: Page, direction: 1 | -1): Promise<number> {
  const { movesPerRun } = CANVAS_WORKER_TOUCH_SCROLL_WORKLOAD;
  return page.evaluate(
    ({ moves, sign }) => {
      const canvas = document.querySelector('#canvas-stage');
      if (!(canvas instanceof HTMLCanvasElement)) {
        throw new Error('Canvas Mode surface is unavailable');
      }
      let accepted = 0;
      const count = () => {
        accepted += 1;
      };
      canvas.addEventListener('pointerdown', count);
      canvas.addEventListener('pointermove', count);
      canvas.addEventListener('pointerup', count);
      const startY = sign > 0 ? canvas.clientHeight * 0.75 : canvas.clientHeight * 0.25;
      const endY = sign > 0 ? canvas.clientHeight * 0.25 : canvas.clientHeight * 0.75;
      const emit = (type: string, y: number) =>
        canvas.dispatchEvent(
          new PointerEvent(type, {
            bubbles: true,
            cancelable: true,
            isPrimary: true,
            pointerId: 1,
            pointerType: 'touch',
            clientX: canvas.clientWidth / 2,
            clientY: y,
          }),
        );
      emit('pointerdown', startY);
      for (let index = 1; index <= moves; index += 1) {
        emit('pointermove', startY + ((endY - startY) * index) / moves);
      }
      emit('pointerup', endY);
      canvas.removeEventListener('pointerdown', count);
      canvas.removeEventListener('pointermove', count);
      canvas.removeEventListener('pointerup', count);
      return accepted;
    },
    { moves: movesPerRun, sign: direction },
  );
}

test.describe('standard Canvas host Worker + shared Pipeline touch-scroll workload', () => {
  test('keeps DOM input live and presents the latest dirty scroll frame without FIFO replay', async ({
    page,
  }) => {
    test.setTimeout(120_000);
    const rendererLogs: string[] = [];
    page.on('console', (message) => rendererLogs.push(message.text()));
    await page.addInitScript(() => window.localStorage.clear());
    await page.goto('/?workload=worker-scroll');

    const canvas = page.locator('#canvas-stage');
    const editContextSupported = await page.evaluate(
      () => typeof (globalThis as { EditContext?: unknown }).EditContext !== 'undefined',
    );
    test.skip(!editContextSupported, 'Canvas Mode requires EditContext');
    await expect(canvas).toBeVisible();
    await expect
      .poll(() => rendererLogs.some((line) => line.includes('selected scene renderer: vello')), {
        timeout: CANVAS_WORKER_TOUCH_SCROLL_WORKLOAD.settleTimeoutMs,
      })
      .toBe(true);

    for (let run = 0; run < CANVAS_WORKER_TOUCH_SCROLL_WORKLOAD.warmupRuns; run += 1) {
      await dispatchTouchScroll(page, 1);
      await expect
        .poll(() => pipelineObservation(page), {
          timeout: CANVAS_WORKER_TOUCH_SCROLL_WORKLOAD.settleTimeoutMs,
        })
        .toMatchObject({ active: false, pending: 0, failure: false });
    }

    const pixelsBefore = await canvas.screenshot();
    let maxPending = 0;
    let observedCoalescing = false;
    for (let run = 0; run < CANVAS_WORKER_TOUCH_SCROLL_WORKLOAD.sampleRuns; run += 1) {
      const mainThreadAccepted = await dispatchTouchScroll(page, run % 2 === 0 ? 1 : -1);
      expect(mainThreadAccepted).toBe(CANVAS_WORKER_TOUCH_SCROLL_WORKLOAD.movesPerRun + 2);
      const duringGpuWork = await pipelineObservation(page);
      maxPending = Math.max(maxPending, duringGpuWork.pending);
      observedCoalescing ||= duringGpuWork.coalesced > 0;
    }

    expect(maxPending).toBeLessThanOrEqual(1);
    expect(observedCoalescing).toBe(true);
    await expect
      .poll(() => pipelineObservation(page), {
        timeout: CANVAS_WORKER_TOUCH_SCROLL_WORKLOAD.settleTimeoutMs,
      })
      .toMatchObject({ active: false, pending: 0, failure: false, dropped: 0 });

    const latestPixels = await canvas.screenshot();
    expect(latestPixels.equals(pixelsBefore)).toBe(false);
    await page.waitForTimeout(CANVAS_WORKER_TOUCH_SCROLL_WORKLOAD.frameBudgetMs60Hz * 2);
    const afterReplayWindow = await canvas.screenshot();
    expect(afterReplayWindow.equals(latestPixels)).toBe(true);
  });
});
