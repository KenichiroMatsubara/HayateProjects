import { describe, expect, it, vi } from 'vitest';
import {
  startTorimiHost,
  type BootTorimiHostOptions,
  type ReloadSubscription,
  type SubscribeReloadOptions,
} from './index.js';
import type { WebHost } from '@torimi/hayate-host';

/**
 * full reload ループの合成ルート（ホスト側）の配線契約テスト。初回 boot と、reload 受信ごとの
 * 再 boot を、boot / subscribe を注入 seam で差し替えて観測する。full reload は毎回**新しい
 * surface**で mount し直す（state は飛ぶ・ADR-0001 / CONTEXT.md「Reload」）。
 */

/** boot seam が返す最小 host。reload teardown の `detach` まで含む（ADR-0124）。 */
function fakeHost(detach: () => void = () => undefined): WebHost {
  return {
    raw: {} as WebHost['raw'],
    requestFrame: () => 0,
    cancelFrame: () => undefined,
    detach,
  };
}

describe('startTorimiHost', () => {
  it('boots once on start, against a freshly acquired canvas', () => {
    const boot = vi.fn(async (_o: BootTorimiHostOptions) => fakeHost());
    const canvas = {} as HTMLCanvasElement;

    startTorimiHost({
      devServerUrl: 'http://dev.example',
      hostProtocolVersion: 1,
      acquireCanvas: () => canvas,
      boot,
      subscribe: () => ({ close() {} }),
    });

    expect(boot).toHaveBeenCalledTimes(1);
    expect(boot.mock.calls[0]![0]).toMatchObject({ devServerUrl: 'http://dev.example', canvas });
  });

  it('re-boots with a fresh canvas each time a reload arrives', () => {
    const boot = vi.fn(async (_o: BootTorimiHostOptions) => fakeHost());
    const canvases = [{ id: 1 } as unknown as HTMLCanvasElement, { id: 2 } as unknown as HTMLCanvasElement];
    const acquireCanvas = vi.fn(() => canvases[acquireCanvas.mock.calls.length - 1]!);
    let onReload: () => void = () => {};

    startTorimiHost({
      devServerUrl: 'http://dev.example',
      hostProtocolVersion: 1,
      acquireCanvas,
      boot,
      subscribe: (o: SubscribeReloadOptions) => {
        onReload = o.onReload;
        return { close() {} };
      },
    });

    expect(boot).toHaveBeenCalledTimes(1);
    expect(boot.mock.calls[0]![0]!.canvas).toBe(canvases[0]);

    onReload();

    expect(boot).toHaveBeenCalledTimes(2);
    expect(boot.mock.calls[1]![0]!.canvas).toBe(canvases[1]);
  });

  it('detaches the previous host before re-booting on reload (ADR-0124 mirror teardown)', async () => {
    const detach = vi.fn();
    const boot = vi.fn(async (_o: BootTorimiHostOptions) => fakeHost(detach));
    let onReload: () => void = () => {};
    let settled = 0;

    startTorimiHost({
      devServerUrl: 'http://dev.example',
      hostProtocolVersion: 1,
      acquireCanvas: () => ({}) as HTMLCanvasElement,
      boot,
      subscribe: (o: SubscribeReloadOptions) => {
        onReload = o.onReload;
        return { close() {} };
      },
      onBootSettled: () => {
        settled += 1;
      },
    });

    // 初回 boot が settle して host（とその detach）が手元に来るのを待つ。
    await vi.waitFor(() => expect(settled).toBe(1));
    expect(detach).not.toHaveBeenCalled();

    // reload → 直前の host を畳んでから再 boot する。
    onReload();
    expect(detach).toHaveBeenCalledTimes(1);
    expect(boot).toHaveBeenCalledTimes(2);
  });

  it('closing the handle unsubscribes from reloads', () => {
    const subscription: ReloadSubscription = { close: vi.fn() };

    const handle = startTorimiHost({
      devServerUrl: 'http://dev.example',
      hostProtocolVersion: 1,
      acquireCanvas: () => ({}) as HTMLCanvasElement,
      boot: async () => fakeHost(),
      subscribe: () => subscription,
    });

    handle.close();

    expect(subscription.close).toHaveBeenCalledTimes(1);
  });
});
