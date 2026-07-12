import { describe, expect, it, vi } from 'vitest';
import type { IRenderer } from '@torimi/tsubame-renderer-protocol';
import { runTsubameApp } from './run.js';
import type { Host, TsubameMount } from './host.js';

/** mount の中身は IRenderer ハンドルを記録するだけの fake。 */
const fakeRenderer = { __tag: 'fake-renderer' } as unknown as IRenderer;

describe('runTsubameApp', () => {
  it('sync Host: createRenderer の renderer を mount へ渡す', () => {
    const createRenderer = vi.fn((): IRenderer => fakeRenderer);
    const mount: TsubameMount = vi.fn();
    runTsubameApp({ createRenderer }, mount);
    expect(createRenderer).toHaveBeenCalledOnce();
    expect(mount).toHaveBeenCalledExactlyOnceWith(fakeRenderer);
  });

  it('dispose は mount の dispose と host.stop を合成する', () => {
    const mountDispose = vi.fn();
    const stop = vi.fn();
    const host: Host = { createRenderer: () => fakeRenderer, stop };
    const dispose = runTsubameApp(host, () => mountDispose);
    expect(mountDispose).not.toHaveBeenCalled();
    expect(stop).not.toHaveBeenCalled();
    dispose();
    expect(mountDispose).toHaveBeenCalledOnce();
    expect(stop).toHaveBeenCalledOnce();
  });

  it('dispose は冪等（二度目は何もしない）', () => {
    const mountDispose = vi.fn();
    const stop = vi.fn();
    const dispose = runTsubameApp({ createRenderer: () => fakeRenderer, stop }, () => mountDispose);
    dispose();
    dispose();
    expect(mountDispose).toHaveBeenCalledOnce();
    expect(stop).toHaveBeenCalledOnce();
  });

  it('async Host: resolve 後に mount する', async () => {
    let resolve!: (r: IRenderer) => void;
    const created = new Promise<IRenderer>((r) => (resolve = r));
    const mount = vi.fn();
    runTsubameApp({ createRenderer: () => created }, mount);
    expect(mount).not.toHaveBeenCalled();
    resolve(fakeRenderer);
    await created;
    expect(mount).toHaveBeenCalledExactlyOnceWith(fakeRenderer);
  });

  it('async Host: resolve 前に dispose されたら mount しない', async () => {
    let resolve!: (r: IRenderer) => void;
    const created = new Promise<IRenderer>((r) => (resolve = r));
    const mount = vi.fn();
    const stop = vi.fn();
    const dispose = runTsubameApp({ createRenderer: () => created, stop }, mount);
    dispose();
    resolve(fakeRenderer);
    await created;
    expect(mount).not.toHaveBeenCalled();
    expect(stop).toHaveBeenCalledOnce();
  });

  it('host.stop 未定義（DOM Host）でも dispose は壊れない', () => {
    const mountDispose = vi.fn();
    const dispose = runTsubameApp({ createRenderer: () => fakeRenderer }, () => mountDispose);
    expect(() => dispose()).not.toThrow();
    expect(mountDispose).toHaveBeenCalledOnce();
  });
});
