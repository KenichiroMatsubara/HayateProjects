import { describe, expect, it, vi } from 'vitest';
import type { IRenderer } from '@torimi/tsubame-renderer-protocol';
import { runTsubameApp } from './run.js';
import type { Host, HostSession, TsubameMount } from './host.js';

const fakeRenderer = { __tag: 'fake-renderer' } as unknown as IRenderer;

function session(dispose = vi.fn()): HostSession {
  return { renderer: fakeRenderer, dispose };
}

describe('runTsubameApp', () => {
  it('sync HostSession の renderer を mount し mounted に settle する', async () => {
    const started = session();
    const start = vi.fn(() => started);
    const host: Host = { start };
    const mount: TsubameMount = vi.fn();

    const handle = runTsubameApp(host, mount);

    expect(start).toHaveBeenCalledOnce();
    expect(mount).toHaveBeenCalledExactlyOnceWith(fakeRenderer);
    await expect(handle.settled).resolves.toEqual({ status: 'mounted' });
  });

  it('async HostSession は resolve 後に mount し mounted に settle する', async () => {
    let resolve!: (value: HostSession) => void;
    const started = new Promise<HostSession>((done) => {
      resolve = done;
    });
    const mount = vi.fn();
    const handle = runTsubameApp({ start: () => started }, mount);
    expect(mount).not.toHaveBeenCalled();

    resolve(session());

    await expect(handle.settled).resolves.toEqual({ status: 'mounted' });
    expect(mount).toHaveBeenCalledExactlyOnceWith(fakeRenderer);
  });

  it('同期 Host.start failure を reject せず failed 値にする', async () => {
    const error = new Error('start failed');

    const handle = runTsubameApp(
      {
        start() {
          throw error;
        },
      },
      vi.fn(),
    );

    await expect(handle.settled).resolves.toEqual({ status: 'failed', error });
  });

  it('非同期 Host.start rejection も failed 値にする', async () => {
    const error = new Error('async start failed');
    const handle = runTsubameApp({ start: () => Promise.reject(error) }, vi.fn());

    await expect(handle.settled).resolves.toEqual({ status: 'failed', error });
  });

  it('framework mount failure は session を破棄して failed に settle する', async () => {
    const error = new Error('mount failed');
    const sessionDispose = vi.fn();
    const handle = runTsubameApp({ start: () => session(sessionDispose) }, () => {
      throw error;
    });

    await expect(handle.settled).resolves.toEqual({ status: 'failed', error });
    expect(sessionDispose).toHaveBeenCalledOnce();
  });

  it('async start 後の mount failure も session を破棄して failed に settle する', async () => {
    const error = new Error('async mount failed');
    const sessionDispose = vi.fn();
    const handle = runTsubameApp(
      { start: () => Promise.resolve(session(sessionDispose)) },
      () => {
        throw error;
      },
    );

    await expect(handle.settled).resolves.toEqual({ status: 'failed', error });
    expect(sessionDispose).toHaveBeenCalledOnce();
  });

  it('async start 中の dispose は即時 settle し、遅着 session を mount しない', async () => {
    let resolve!: (value: HostSession) => void;
    const started = new Promise<HostSession>((done) => {
      resolve = done;
    });
    const sessionDispose = vi.fn();
    const mount = vi.fn();
    const handle = runTsubameApp({ start: () => started }, mount);

    handle.dispose();

    await expect(handle.settled).resolves.toEqual({ status: 'disposed' });
    resolve(session(sessionDispose));
    await started;
    await Promise.resolve();
    expect(mount).not.toHaveBeenCalled();
    expect(sessionDispose).toHaveBeenCalledOnce();
  });

  it('通常 dispose は framework、session の順で各一度だけ破棄する', async () => {
    const order: string[] = [];
    const mountDispose = vi.fn(() => order.push('framework'));
    const sessionDispose = vi.fn(() => order.push('session'));
    const handle = runTsubameApp(
      { start: () => session(sessionDispose) },
      () => mountDispose,
    );
    await handle.settled;

    handle.dispose();
    handle.dispose();

    expect(order).toEqual(['framework', 'session']);
    expect(mountDispose).toHaveBeenCalledOnce();
    expect(sessionDispose).toHaveBeenCalledOnce();
  });

  it('一方の cleanup failure で残りの cleanup を抑止しない', async () => {
    const order: string[] = [];
    const sessionDispose = vi.fn(() => order.push('session'));
    const handle = runTsubameApp(
      { start: () => session(sessionDispose) },
      () => () => {
        order.push('framework');
        throw new Error('framework cleanup failed');
      },
    );
    await handle.settled;

    expect(() => handle.dispose()).not.toThrow();
    expect(order).toEqual(['framework', 'session']);
  });
});
