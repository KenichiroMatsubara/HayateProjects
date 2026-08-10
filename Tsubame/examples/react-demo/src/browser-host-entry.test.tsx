// @vitest-environment happy-dom

import { beforeEach, describe, expect, it, vi } from 'vitest';

const harness = vi.hoisted(() => ({
  events: [] as string[],
}));

vi.mock('@torimi/tsubame-browser-host', () => ({
  createBrowserHost: ({
    dom,
    canvas,
  }: {
    dom: HTMLElement;
    canvas: HTMLCanvasElement;
  }) => ({
    start: () => {
      const domHidden = dom.hidden;
      const canvasHidden = canvas.hidden;
      const target =
        new URLSearchParams(window.location.search).get('renderer') === 'dom'
          ? 'dom'
          : 'hayate';
      dom.hidden = target === 'hayate';
      canvas.hidden = target === 'dom';
      harness.events.push(`host.start:${target}`);
      return {
        renderer: {},
        dispose: () => {
          if (target === 'hayate') {
            harness.events.push('renderer.stop', 'worker.detach');
          } else {
            harness.events.push('session.dispose');
          }
          dom.hidden = domHidden;
          canvas.hidden = canvasHidden;
        },
      };
    },
  }),
}));

vi.mock('@torimi/tsubame-react', () => ({
  renderTsubame: () => {
    harness.events.push('react.mount');
    return () => harness.events.push('react.dispose');
  },
}));

beforeEach(() => {
  vi.resetModules();
  harness.events.length = 0;
  document.body.innerHTML =
    '<div id="dom-host" hidden></div><canvas id="canvas-stage" hidden></canvas>';
  window.history.replaceState(null, '', '/?renderer=dom');
});

describe('React browser entry', () => {
  it('mounts the DOM target and releases the framework, session, and visibility on pagehide', async () => {
    const entry = await import('./main');

    await expect(entry.appHandle.settled).resolves.toEqual({ status: 'mounted' });
    expect(harness.events).toEqual(['host.start:dom', 'react.mount']);
    expect(document.getElementById('dom-host')?.hidden).toBe(false);
    expect(document.getElementById('canvas-stage')?.hidden).toBe(true);

    window.dispatchEvent(new Event('pagehide'));

    expect(harness.events).toEqual([
      'host.start:dom',
      'react.mount',
      'react.dispose',
      'session.dispose',
    ]);
    expect(document.getElementById('dom-host')?.hidden).toBe(true);
    expect(document.getElementById('canvas-stage')?.hidden).toBe(true);
  });

  it('mounts the Hayate target and detaches its Worker while restoring both surfaces', async () => {
    window.history.replaceState(null, '', '/');
    const entry = await import('./main');

    await expect(entry.appHandle.settled).resolves.toEqual({ status: 'mounted' });
    expect(harness.events).toEqual(['host.start:hayate', 'react.mount']);
    expect(document.getElementById('dom-host')?.hidden).toBe(true);
    expect(document.getElementById('canvas-stage')?.hidden).toBe(false);

    entry.appHandle.dispose();

    expect(harness.events).toEqual([
      'host.start:hayate',
      'react.mount',
      'react.dispose',
      'renderer.stop',
      'worker.detach',
    ]);
    expect(document.getElementById('dom-host')?.hidden).toBe(true);
    expect(document.getElementById('canvas-stage')?.hidden).toBe(true);
  });
});
