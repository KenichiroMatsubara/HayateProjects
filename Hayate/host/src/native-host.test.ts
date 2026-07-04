import { describe, expect, it, vi } from 'vitest';
import { createHayateNativeHost } from './native.js';
import type { RawHayate } from './raw-hayate.js';

// native は WASM を巻き込まない：ネイティブ Hayate（Hermes/JSI, ADR-0112）が注入した
// RawHayate を受け取り、frame-clock を「ネイティブ vsync が 1 フレームずつ駆動する
// pump」として供給する。`requestFrame` は最新コールバックを保持するだけで自走しない。
const stubRaw = { render: () => undefined } as unknown as RawHayate;

describe('createHayateNativeHost', () => {
  it('exposes the injected raw renderer for the composition root', () => {
    const host = createHayateNativeHost(stubRaw);
    expect(host.raw).toBe(stubRaw);
  });

  it('does not run a requested frame until the native vsync pump fires', () => {
    const host = createHayateNativeHost(stubRaw);
    const cb = vi.fn();

    host.requestFrame(cb);
    expect(cb).not.toHaveBeenCalled();

    host.pumpFrame(16);
    expect(cb).toHaveBeenCalledTimes(1);
    expect(cb).toHaveBeenCalledWith(16);
  });

  it('runs each held callback at most once per pump', () => {
    const host = createHayateNativeHost(stubRaw);
    const cb = vi.fn();

    host.requestFrame(cb);
    host.pumpFrame(16);
    host.pumpFrame(32);

    expect(cb).toHaveBeenCalledTimes(1);
  });

  it('chains frames: a callback re-armed during a pump runs on the next pump', () => {
    const host = createHayateNativeHost(stubRaw);
    const order: number[] = [];
    const second = (ts: number) => order.push(ts);
    const first = (ts: number) => {
      order.push(ts);
      host.requestFrame(second);
    };

    host.requestFrame(first);
    host.pumpFrame(16);
    host.pumpFrame(32);

    expect(order).toEqual([16, 32]);
  });

  it('cancelFrame drops the pending callback so the next pump is a no-op', () => {
    const host = createHayateNativeHost(stubRaw);
    const cb = vi.fn();

    const handle = host.requestFrame(cb);
    host.cancelFrame(handle);
    host.pumpFrame(16);

    expect(cb).not.toHaveBeenCalled();
  });

  it('stop halts frame driving', () => {
    const host = createHayateNativeHost(stubRaw);
    const cb = vi.fn();

    host.requestFrame(cb);
    host.stop();
    host.pumpFrame(16);

    expect(cb).not.toHaveBeenCalled();
  });

  // native の on-demand ループ（app_tsubame.rs）には web の requestAnimationFrame の
  // ような自走クロックが無く、`request_pump` の native wake が無いと armed になった
  // フレームが二度と pump されない（タップ/スクロールが一切効かなくなる回帰）。
  it('notifies native that a frame is armed every time requestFrame is called', () => {
    const requestPump = vi.fn();
    const raw = { ...stubRaw, request_pump: requestPump } as unknown as RawHayate;
    const host = createHayateNativeHost(raw);

    host.requestFrame(vi.fn());
    expect(requestPump).toHaveBeenCalledTimes(1);

    // 再武装（例: click ハンドラの mutation が scheduleFrame を叩く継続フレーム）でも
    // 都度呼ぶ — native はこれが無いと再武装を知る術が無い。
    host.pumpFrame(16);
    host.requestFrame(vi.fn());
    expect(requestPump).toHaveBeenCalledTimes(2);
  });

  it('does not throw when the host has no request_pump (optional, e.g. older hosts)', () => {
    const host = createHayateNativeHost(stubRaw);
    expect(() => host.requestFrame(vi.fn())).not.toThrow();
  });
});
