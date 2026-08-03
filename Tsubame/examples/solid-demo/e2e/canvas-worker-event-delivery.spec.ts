import { expect, test, type Page } from '@playwright/test';

const CANVAS_WORKER_EVENT_DELIVERY_WORKLOAD = {
  viewport: { width: 1280, height: 720 },
  deviceScaleFactor: 1,
  cssGalleryTap: { x: 920, y: 32 },
  tasksTap: { x: 827, y: 32 },
  neutralPointer: { x: 16, y: 700 },
  contentClip: { x: 0, y: 64, width: 1280, height: 656 },
  settleTimeoutMs: 10_000,
  staleReplayWindowMs: 100,
  pixelChannelTolerance: 8,
  minPageChangeRatio: 0.02,
  maxStablePixelChangeRatio: 0.002,
} as const;

test.use({
  viewport: CANVAS_WORKER_EVENT_DELIVERY_WORKLOAD.viewport,
  deviceScaleFactor: CANVAS_WORKER_EVENT_DELIVERY_WORKLOAD.deviceScaleFactor,
});

interface PipelineObservation {
  active: boolean;
  pending: number;
  failure: boolean;
}

async function pipelineObservation(page: Page): Promise<PipelineObservation> {
  return page.evaluate(async () => {
    const host = (
      window as unknown as {
        __tsubameBrowserHostInspection?: {
          pipelineObservation(): Promise<PipelineObservation>;
        };
      }
    ).__tsubameBrowserHostInspection;
    if (!host) throw new Error('standard Canvas Worker host is not mounted');
    return host.pipelineObservation();
  });
}

async function waitForIdleContentPixels(page: Page): Promise<Buffer> {
  await expect
    .poll(() => pipelineObservation(page), {
      timeout: CANVAS_WORKER_EVENT_DELIVERY_WORKLOAD.settleTimeoutMs,
    })
    .toMatchObject({ active: false, pending: 0, failure: false });
  return page.screenshot({
    clip: CANVAS_WORKER_EVENT_DELIVERY_WORKLOAD.contentClip,
  });
}

async function changedPixelRatio(
  page: Page,
  first: Buffer,
  second: Buffer,
): Promise<number> {
  return page.evaluate(
    async ({ firstPng, secondPng, channelTolerance }) => {
      const decode = async (bytes: number[]) => {
        const bitmap = await createImageBitmap(new Blob([Uint8Array.from(bytes)]));
        const surface = new OffscreenCanvas(bitmap.width, bitmap.height);
        const context = surface.getContext('2d', { willReadFrequently: true });
        if (!context) throw new Error('2D pixel comparison context is unavailable');
        context.drawImage(bitmap, 0, 0);
        bitmap.close();
        return context.getImageData(0, 0, surface.width, surface.height);
      };
      const [a, b] = await Promise.all([decode(firstPng), decode(secondPng)]);
      if (a.width !== b.width || a.height !== b.height) return 1;
      let changed = 0;
      for (let index = 0; index < a.data.length; index += 4) {
        const channelChanged =
          Math.abs(a.data[index]! - b.data[index]!) > channelTolerance ||
          Math.abs(a.data[index + 1]! - b.data[index + 1]!) > channelTolerance ||
          Math.abs(a.data[index + 2]! - b.data[index + 2]!) > channelTolerance ||
          Math.abs(a.data[index + 3]! - b.data[index + 3]!) > channelTolerance;
        if (channelChanged) changed += 1;
      }
      return changed / (a.data.length / 4);
    },
    {
      firstPng: Array.from(first),
      secondPng: Array.from(second),
      channelTolerance: CANVAS_WORKER_EVENT_DELIVERY_WORKLOAD.pixelChannelTolerance,
    },
  );
}

test.describe('standard Canvas Worker Event Delivery', () => {
  test('switches CSS Gallery and Tasks through Worker click delivery without stale replay', async ({
    page,
  }) => {
    test.setTimeout(120_000);
    const rendererLogs: string[] = [];
    page.on('console', (message) => rendererLogs.push(message.text()));
    await page.addInitScript(() => window.localStorage.clear());
    await page.goto('/');

    const canvas = page.locator('#canvas-stage');
    const editContextSupported = await page.evaluate(
      () => typeof (globalThis as { EditContext?: unknown }).EditContext !== 'undefined',
    );
    test.skip(!editContextSupported, 'Canvas Mode requires EditContext');
    await expect(canvas).toBeVisible();
    await expect
      .poll(() => rendererLogs.some((line) => line.includes('selected scene renderer: vello')), {
        timeout: CANVAS_WORKER_EVENT_DELIVERY_WORKLOAD.settleTimeoutMs,
      })
      .toBe(true);

    const tasksPixels = await waitForIdleContentPixels(page);
    await page.mouse.click(
      CANVAS_WORKER_EVENT_DELIVERY_WORKLOAD.cssGalleryTap.x,
      CANVAS_WORKER_EVENT_DELIVERY_WORKLOAD.cssGalleryTap.y,
    );
    await page.mouse.move(
      CANVAS_WORKER_EVENT_DELIVERY_WORKLOAD.neutralPointer.x,
      CANVAS_WORKER_EVENT_DELIVERY_WORKLOAD.neutralPointer.y,
    );
    await expect
      .poll(
        async () =>
          changedPixelRatio(page, tasksPixels, await waitForIdleContentPixels(page)),
        {
          timeout: CANVAS_WORKER_EVENT_DELIVERY_WORKLOAD.settleTimeoutMs,
        },
      )
      .toBeGreaterThan(CANVAS_WORKER_EVENT_DELIVERY_WORKLOAD.minPageChangeRatio);
    const galleryPixels = await waitForIdleContentPixels(page);

    await page.waitForTimeout(CANVAS_WORKER_EVENT_DELIVERY_WORKLOAD.staleReplayWindowMs);
    expect(
      await changedPixelRatio(
        page,
        galleryPixels,
        await page.screenshot({
          clip: CANVAS_WORKER_EVENT_DELIVERY_WORKLOAD.contentClip,
        }),
      ),
    ).toBeLessThanOrEqual(
      CANVAS_WORKER_EVENT_DELIVERY_WORKLOAD.maxStablePixelChangeRatio,
    );

    await page.mouse.click(
      CANVAS_WORKER_EVENT_DELIVERY_WORKLOAD.tasksTap.x,
      CANVAS_WORKER_EVENT_DELIVERY_WORKLOAD.tasksTap.y,
    );
    await page.mouse.move(
      CANVAS_WORKER_EVENT_DELIVERY_WORKLOAD.neutralPointer.x,
      CANVAS_WORKER_EVENT_DELIVERY_WORKLOAD.neutralPointer.y,
    );
    await expect
      .poll(
        async () =>
          changedPixelRatio(page, tasksPixels, await waitForIdleContentPixels(page)),
        {
          timeout: CANVAS_WORKER_EVENT_DELIVERY_WORKLOAD.settleTimeoutMs,
        },
      )
      .toBeLessThanOrEqual(
        CANVAS_WORKER_EVENT_DELIVERY_WORKLOAD.maxStablePixelChangeRatio,
      );
  });
});
