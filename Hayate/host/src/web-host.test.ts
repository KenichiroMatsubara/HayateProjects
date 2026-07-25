import { describe, expect, it, vi } from 'vitest';
import {
  createHayateWebHost,
  type CreateHayateWebHostOptions,
} from './index.js';
import type { MainToWorker, WorkerToMain } from './worker-host.js';

function fakeWorkerTransport(
  bootReply: WorkerToMain = { kind: 'ready' },
) {
  const sent: MainToWorker[] = [];
  let onMessage: ((message: WorkerToMain) => void) | undefined;
  const transport = {
    postMessage: (message: MainToWorker) => {
      sent.push(message);
      if (message.kind === 'init') {
        queueMicrotask(() => onMessage?.(bootReply));
      } else if (message.kind === 'detach') {
        queueMicrotask(() => onMessage?.({ kind: 'detached' }));
      }
    },
    onMessage: (callback: (message: WorkerToMain) => void) => {
      onMessage = callback;
    },
    terminate: vi.fn(),
  };
  return {
    transport,
    sent,
    emit: (message: WorkerToMain) => onMessage?.(message),
  };
}

const canvas = {
  width: 800,
  height: 600,
  style: {},
  addEventListener: () => {},
  removeEventListener: () => {},
  getBoundingClientRect: () => ({ width: 0, height: 0 }),
} as unknown as HTMLCanvasElement;

describe('createHayateWebHost Worker cutover', () => {
  it('boots exactly one Worker by default without a runtime selection flag', async () => {
    const { transport } = fakeWorkerTransport();
    const spawnWorker = vi.fn(() => transport);

    await createHayateWebHost(canvas, {
      spawnWorker,
      transferControlToOffscreen: () => ({ token: 'offscreen' }),
    });

    expect(spawnWorker).toHaveBeenCalledTimes(1);
  });

  it('routes app mutations and frames through the same Worker execution path', async () => {
    const { transport, sent } = fakeWorkerTransport();
    const host = await createHayateWebHost(canvas, {
      spawnWorker: () => transport,
      transferControlToOffscreen: () => ({ token: 'offscreen' }),
    });

    host.raw.element_create(1, 0);
    host.raw.set_root(1);
    host.raw.render(16);

    expect(sent.slice(1)).toEqual([
      {
        kind: 'mutation',
        command: { kind: 'element-create', id: 1, elementKind: 0 },
      },
      { kind: 'mutation', command: { kind: 'set-root', id: 1 } },
      { kind: 'frame', timestampMs: 16 },
    ]);
  });

  it('applies development tuning inside the Worker path', async () => {
    const { transport, sent } = fakeWorkerTransport();
    await createHayateWebHost(canvas, {
      tuning: '{"profile":"android"}',
      spawnWorker: () => transport,
      transferControlToOffscreen: () => ({}),
    });

    expect(sent).toContainEqual({
      kind: 'mutation',
      command: { kind: 'tuning', json: '{"profile":"android"}' },
    });
  });

  it('exposes shared pipeline observations from the Worker-owned Rust pipeline', async () => {
    const { transport, sent, emit } = fakeWorkerTransport();
    const host = await createHayateWebHost(canvas, {
      spawnWorker: () => transport,
      transferControlToOffscreen: () => ({}),
    });

    const pending = host.pipelineObservation();
    const request = sent.find(
      (message) => message.kind === 'observe-pipeline',
    ) as Extract<MainToWorker, { kind: 'observe-pipeline' }>;
    emit({
      kind: 'pipeline-observation',
      requestId: request.requestId,
      observation: {
        accepted: 80,
        coalesced: 72,
        dropped: 0,
        active: true,
        pending: 1,
        failure: false,
      },
    });

    await expect(pending).resolves.toMatchObject({
      accepted: 80,
      coalesced: 72,
      pending: 1,
      failure: false,
    });
  });

  it('returns a typed renderer boot failure without loading a main-thread fallback', async () => {
    const { transport } = fakeWorkerTransport({
      kind: 'boot-failure',
      failure: {
        code: 'renderer-init-failed',
        message: 'OffscreenCanvas renderer unavailable',
      },
    });

    await expect(
      createHayateWebHost(canvas, {
        spawnWorker: () => transport,
        transferControlToOffscreen: () => ({ token: 'offscreen' }),
      }),
    ).rejects.toMatchObject({
      name: 'WorkerBootError',
      code: 'renderer-init-failed',
    });
    expect(transport.terminate).toHaveBeenCalledTimes(1);
  });

  it('returns a typed boot failure when OffscreenCanvas transfer is unavailable', async () => {
    const { transport } = fakeWorkerTransport();

    await expect(
      createHayateWebHost(canvas, {
        spawnWorker: () => transport,
      }),
    ).rejects.toMatchObject({
      name: 'WorkerBootError',
      code: 'offscreen-canvas-unavailable',
    });
    expect(transport.terminate).toHaveBeenCalledTimes(1);
  });

  it('ignores removed legacy flags instead of restoring the main-thread Canvas path', async () => {
    const { transport } = fakeWorkerTransport();
    const spawnWorker = vi.fn(() => transport);
    const legacy = {
      workerEngine: false,
      locationSearch: '?hayate-engine=off',
      spawnWorker,
      transferControlToOffscreen: () => ({}),
    } as CreateHayateWebHostOptions;

    await createHayateWebHost(canvas, legacy);

    expect(spawnWorker).toHaveBeenCalledTimes(1);
  });

  it('waits for the Worker shutdown barrier before terminating it', async () => {
    const { transport } = fakeWorkerTransport();
    const host = await createHayateWebHost(canvas, {
      spawnWorker: () => transport,
      transferControlToOffscreen: () => ({}),
    });

    host.detach();
    expect(transport.terminate).not.toHaveBeenCalled();
    await Promise.resolve();
    expect(transport.terminate).toHaveBeenCalledTimes(1);
  });
});
