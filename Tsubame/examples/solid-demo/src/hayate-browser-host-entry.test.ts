// @vitest-environment happy-dom

import { beforeEach, describe, expect, it, vi } from 'vitest';

const harness = vi.hoisted(() => ({
  events: [] as string[],
  observation: {
    accepted: 1,
    coalesced: 0,
    dropped: 0,
    active: false,
    pending: 0,
    failure: false,
  },
}));

vi.mock('@torimi/tsubame-browser-host', () => ({
  createBrowserHost: ({
    dom,
    canvas,
  }: {
    dom: HTMLElement;
    canvas: HTMLCanvasElement;
  }) => ({
    inspection: () => ({
      target: 'hayate' as const,
      pipelineObservation: async () => harness.observation,
    }),
    start: () => {
      dom.hidden = true;
      canvas.hidden = false;
      harness.events.push('renderer.start');
      return {
        renderer: {},
        dispose: () => harness.events.push('session.dispose'),
      };
    },
  }),
}));

vi.mock('@torimi/tsubame-solid', () => ({
  renderTsubame: () => {
    harness.events.push('solid.mount');
    return () => harness.events.push('solid.dispose');
  },
}));

beforeEach(() => {
  harness.events.length = 0;
  document.body.innerHTML =
    '<div id="dom-host" hidden></div><canvas id="canvas-stage" hidden></canvas>';
  window.history.replaceState(null, '', '/');
});

describe('Solid Hayate browser composition', () => {
  it('settles after drawing starts and completely tears down its inspection and session', async () => {
    const entry = await import('./main');

    await expect(entry.appHandle.settled).resolves.toEqual({ status: 'mounted' });
    expect(harness.events).toEqual(['renderer.start', 'solid.mount']);
    const inspection = (
      window as unknown as {
        __tsubameBrowserHostInspection?: {
          readonly target: string;
          pipelineObservation(): Promise<typeof harness.observation>;
        };
      }
    ).__tsubameBrowserHostInspection;
    expect(inspection?.target).toBe('hayate');
    await expect(inspection?.pipelineObservation()).resolves.toEqual(
      harness.observation,
    );

    entry.appHandle.dispose();
    entry.appHandle.dispose();

    expect(harness.events).toEqual([
      'renderer.start',
      'solid.mount',
      'solid.dispose',
      'session.dispose',
    ]);
    expect(
      (
        window as unknown as {
          __tsubameBrowserHostInspection?: unknown;
        }
      ).__tsubameBrowserHostInspection,
    ).toBeUndefined();
  });
});
