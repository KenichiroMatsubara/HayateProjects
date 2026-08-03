// @vitest-environment happy-dom

import type { HostSession } from '@torimi/tsubame-app';
import type { IRenderer } from '@torimi/tsubame-renderer-protocol';
import { describe, expect, it, vi } from 'vitest';
import { createBrowserHost } from './browser-host.js';

const renderer = {} as IRenderer;

function surfaces(): { dom: HTMLDivElement; canvas: HTMLCanvasElement } {
  return {
    dom: document.createElement('div'),
    canvas: document.createElement('canvas'),
  };
}

describe('Browser Host DOM target', () => {
  it('selects explicit DOM without evaluating Hayate and restores both surfaces idempotently', () => {
    const { dom, canvas } = surfaces();
    dom.hidden = true;
    canvas.hidden = false;
    const startHayate = vi.fn<() => HostSession>();
    const host = createBrowserHost({
      dom,
      canvas,
      environment: { search: '?renderer=dom', hasEditContext: true },
      createDomRenderer: () => renderer,
      startHayate,
    });

    const session = host.start() as HostSession;

    expect(session.renderer).toBe(renderer);
    expect(startHayate).not.toHaveBeenCalled();
    expect(dom.hidden).toBe(false);
    expect(canvas.hidden).toBe(true);

    session.dispose();
    session.dispose();
    expect(dom.hidden).toBe(true);
    expect(canvas.hidden).toBe(false);
  });

  it('uses DOM when EditContext is absent', () => {
    const { dom, canvas } = surfaces();
    const startHayate = vi.fn<() => HostSession>();
    const session = createBrowserHost({
      dom,
      canvas,
      environment: { search: '?renderer=anything', hasEditContext: false },
      createDomRenderer: () => renderer,
      startHayate,
    }).start() as HostSession;

    expect(session.renderer).toBe(renderer);
    expect(startHayate).not.toHaveBeenCalled();
  });
});

describe('Browser Host Hayate target boundary', () => {
  it('treats every non-DOM renderer query value as Hayate without converting a backend enum', () => {
    for (const value of ['vello', 'tiny-skia', 'auto', 'future-backend']) {
      const { dom, canvas } = surfaces();
      const hayateSession: HostSession = { renderer, dispose() {} };
      const startHayate = vi.fn(() => hayateSession);

      const session = createBrowserHost({
        dom,
        canvas,
        environment: { search: `?renderer=${value}`, hasEditContext: true },
        createDomRenderer: () => {
          throw new Error('DOM renderer must not be evaluated');
        },
        startHayate,
      }).start() as HostSession;

      expect(session.renderer).toBe(renderer);
      expect(startHayate).toHaveBeenCalledOnce();
      expect(startHayate).toHaveBeenCalledWith(canvas);
    }
  });
});
