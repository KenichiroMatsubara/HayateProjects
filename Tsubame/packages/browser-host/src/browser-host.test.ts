// @vitest-environment happy-dom

import type { WebHost } from '@torimi/hayate-host';
import { runTsubameApp, type HostSession } from '@torimi/tsubame-app';
import type { IRenderer } from '@torimi/tsubame-renderer-protocol';
import { describe, expect, it, vi } from 'vitest';
import { createBrowserHost } from './browser-host.js';

const renderer = {} as IRenderer;

function unreachableWebHost(): Promise<WebHost> {
  throw new Error('Hayate Web Host must not be evaluated');
}

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
    const createHayateWebHost = vi.fn(unreachableWebHost);
    const host = createBrowserHost({
      dom,
      canvas,
      environment: { search: '?renderer=dom', hasEditContext: true },
      createDomRenderer: () => renderer,
      createHayateWebHost,
    });

    const session = host.start() as HostSession;

    expect(session.renderer).toBe(renderer);
    expect(createHayateWebHost).not.toHaveBeenCalled();
    expect(dom.hidden).toBe(false);
    expect(canvas.hidden).toBe(true);

    session.dispose();
    session.dispose();
    expect(dom.hidden).toBe(true);
    expect(canvas.hidden).toBe(false);
  });

  it('uses DOM when EditContext is absent', () => {
    const { dom, canvas } = surfaces();
    const createHayateWebHost = vi.fn(unreachableWebHost);
    const session = createBrowserHost({
      dom,
      canvas,
      environment: { search: '?renderer=anything', hasEditContext: false },
      createDomRenderer: () => renderer,
      createHayateWebHost,
    }).start() as HostSession;

    expect(session.renderer).toBe(renderer);
    expect(createHayateWebHost).not.toHaveBeenCalled();
  });

  it('does not evaluate a tuning source for the DOM target', () => {
    const { dom, canvas } = surfaces();
    const loadTuning = vi.fn(() => {
      throw new Error('DOM must not load tuning');
    });

    createBrowserHost({
      dom,
      canvas,
      environment: { search: '?renderer=dom', hasEditContext: true },
      tuning: { kind: 'optional-url', url: '/tuning.json' },
      loadTuning,
      createDomRenderer: () => renderer,
    }).start();

    expect(loadTuning).not.toHaveBeenCalled();
  });
});

describe('Browser Host Hayate target boundary', () => {
  it('treats every non-DOM renderer query value as Hayate without converting a backend enum', async () => {
    for (const value of ['vello', 'tiny-skia', 'auto', 'future-backend']) {
      const { dom, canvas } = surfaces();
      const webHost = {
        raw: {} as never,
        requestFrame: () => 1,
        cancelFrame() {},
        pipelineObservation: async () => ({}) as never,
        detach() {},
      } satisfies WebHost;
      const createHayateWebHost = vi.fn(async () => webHost);
      const startedRenderer = {
        ...renderer,
        start() {},
        stop() {},
      } as IRenderer & { start(): void; stop(): void };

      const session = createBrowserHost({
        dom,
        canvas,
        environment: { search: `?renderer=${value}`, hasEditContext: true },
        createDomRenderer: () => {
          throw new Error('DOM renderer must not be evaluated');
        },
        createHayateWebHost,
        createHayateRenderer: () => startedRenderer,
      }).start() as Promise<HostSession>;

      expect((await session).renderer).toBe(startedRenderer);
      expect(createHayateWebHost).toHaveBeenCalledOnce();
      expect(createHayateWebHost).toHaveBeenCalledWith(canvas, {
        tuning: undefined,
      });
    }
  });

  it('owns surface, tuning, Worker, and renderer as one reverse-cleaned transaction', async () => {
    const { dom, canvas } = surfaces();
    dom.hidden = false;
    canvas.hidden = true;
    const events: string[] = [];
    const hayateRenderer = {
      start() {
        events.push('renderer.start');
      },
      stop() {
        events.push('renderer.stop');
        throw new Error('stop failed');
      },
    } as unknown as IRenderer & { start(): void; stop(): void };

    const host = createBrowserHost({
      dom,
      canvas,
      environment: { search: '', hasEditContext: true },
      tuning: { kind: 'inline', json: '{"motion":false}' },
      loadTuning: async (source) => {
        expect(dom.hidden).toBe(true);
        expect(canvas.hidden).toBe(false);
        events.push(`tuning:${source.kind}`);
        return '{"motion":false}';
      },
      createHayateWebHost: async (_surface, options) => {
        events.push(`webHost:${options?.tuning}`);
        return {
          raw: {} as never,
          requestFrame: () => 1,
          cancelFrame() {},
          pipelineObservation: async () => ({
            accepted: 0,
            coalesced: 0,
            dropped: 0,
            pending: 0,
            active: false,
            failure: false,
          }),
          detach() {
            events.push('webHost.detach');
          },
        };
      },
      createHayateRenderer: () => {
        events.push('renderer.create');
        return hayateRenderer;
      },
    });

    const session = await host.start();
    expect(events).toEqual([
      'tuning:inline',
      'webHost:{"motion":false}',
      'renderer.create',
      'renderer.start',
    ]);

    expect(() => session.dispose()).not.toThrow();
    session.dispose();
    expect(events.slice(4)).toEqual(['renderer.stop', 'webHost.detach']);
    expect(dom.hidden).toBe(false);
    expect(canvas.hidden).toBe(true);
  });

  it('exposes only target and the active Web Host pipeline observation', async () => {
    const { dom, canvas } = surfaces();
    const observation = {
      accepted: 4,
      coalesced: 0,
      dropped: 0,
      pending: 0,
      active: false,
      failure: false,
    };
    const webHost = {
      observation,
      raw: { secret: 'raw' } as never,
      requestFrame: () => 1,
      cancelFrame() {},
      pipelineObservation() {
        return Promise.resolve(this.observation);
      },
      detach() {},
    };
    const host = createBrowserHost({
      dom,
      canvas,
      environment: { search: '', hasEditContext: true },
      createHayateWebHost: async () => webHost,
      createHayateRenderer: () => ({
        ...renderer,
        start() {},
        stop() {},
      }),
    });

    await host.start();
    const inspection = host.inspection();

    expect(Object.keys(inspection).sort()).toEqual([
      'pipelineObservation',
      'target',
    ]);
    expect(inspection.target).toBe('hayate');
    await expect(inspection.pipelineObservation()).resolves.toEqual(observation);
    expect(inspection).not.toHaveProperty('raw');
    expect(inspection).not.toHaveProperty('renderer');
    expect(inspection).not.toHaveProperty('worker');
  });

  it('keeps optional URL tuning best-effort and performs no fetch by default', async () => {
    const received: Array<string | undefined> = [];
    const fetchTuning = vi.fn(async () => ({
      ok: true,
      text: async () => '{"profile":"success"}',
    }));
    const start = async (
      tuning:
        | { kind: 'none' }
        | { kind: 'inline'; json: string }
        | { kind: 'optional-url'; url: string }
        | undefined,
      fetcher = fetchTuning,
    ) => {
      const { dom, canvas } = surfaces();
      const session = await createBrowserHost({
        dom,
        canvas,
        environment: { search: '', hasEditContext: true },
        ...(tuning === undefined ? {} : { tuning }),
        fetchTuning: fetcher,
        createHayateWebHost: async (_surface, options) => {
          received.push(options?.tuning);
          return {
            raw: {} as never,
            requestFrame: () => 1,
            cancelFrame() {},
            pipelineObservation: async () => ({}) as never,
            detach() {},
          };
        },
        createHayateRenderer: () => ({
          ...renderer,
          start() {},
          stop() {},
        }),
      }).start();
      session.dispose();
    };

    await start(undefined);
    await start({ kind: 'inline', json: '{"profile":"inline"}' });
    await start({ kind: 'optional-url', url: '/tuning.json' });
    await start(
      { kind: 'optional-url', url: '/missing.json' },
      vi.fn(async () => ({ ok: false, text: async () => 'ignored' })),
    );
    await start(
      { kind: 'optional-url', url: '/offline.json' },
      vi.fn(async () => {
        throw new Error('offline');
      }),
    );

    expect(received).toEqual([
      undefined,
      '{"profile":"inline"}',
      '{"profile":"success"}',
      undefined,
      undefined,
    ]);
    expect(fetchTuning).toHaveBeenCalledOnce();
    expect(fetchTuning).toHaveBeenCalledWith('/tuning.json');
  });

  it('cleans every acquired resource for faults at each start stage', async () => {
    const stages = ['tuning', 'web-host', 'renderer-create', 'renderer-start'] as const;

    for (const stage of stages) {
      const { dom, canvas } = surfaces();
      dom.hidden = false;
      canvas.hidden = true;
      const events: string[] = [];
      const host = createBrowserHost({
        dom,
        canvas,
        environment: { search: '', hasEditContext: true },
        loadTuning: async () => {
          if (stage === 'tuning') throw new Error(stage);
          return undefined;
        },
        createHayateWebHost: async () => {
          if (stage === 'web-host') throw new Error(stage);
          return {
            raw: {} as never,
            requestFrame: () => 1,
            cancelFrame() {},
            pipelineObservation: async () => ({}) as never,
            detach() {
              events.push('detach');
            },
          };
        },
        createHayateRenderer: () => {
          if (stage === 'renderer-create') throw new Error(stage);
          return {
            ...renderer,
            start() {
              if (stage === 'renderer-start') throw new Error(stage);
            },
            stop() {
              events.push('stop');
            },
          };
        },
      });

      await expect(host.start()).rejects.toThrow(stage);
      expect(dom.hidden).toBe(false);
      expect(canvas.hidden).toBe(true);
      expect(events).toEqual(
        stage === 'renderer-start'
          ? ['stop', 'detach']
          : stage === 'renderer-create'
            ? ['detach']
            : [],
      );
    }
  });

  it('releases a late-resolved Hayate session after AppHandle is disposed', async () => {
    const { dom, canvas } = surfaces();
    dom.hidden = false;
    canvas.hidden = true;
    const events: string[] = [];
    let resolveWebHost!: (host: WebHost) => void;
    const pendingWebHost = new Promise<WebHost>((resolve) => {
      resolveWebHost = resolve;
    });
    const host = createBrowserHost({
      dom,
      canvas,
      environment: { search: '', hasEditContext: true },
      createHayateWebHost: () => pendingWebHost,
      createHayateRenderer: () => ({
        ...renderer,
        start() {
          events.push('start');
        },
        stop() {
          events.push('stop');
        },
      }),
    });
    const mount = vi.fn();
    const handle = runTsubameApp(host, mount);

    handle.dispose();
    await expect(handle.settled).resolves.toEqual({ status: 'disposed' });
    resolveWebHost({
      raw: {} as never,
      requestFrame: () => 1,
      cancelFrame() {},
      pipelineObservation: async () => ({}) as never,
      detach() {
        events.push('detach');
      },
    });

    await vi.waitFor(() => {
      expect(events).toEqual(['start', 'stop', 'detach']);
    });
    expect(mount).not.toHaveBeenCalled();
    expect(dom.hidden).toBe(false);
    expect(canvas.hidden).toBe(true);
  });
});
