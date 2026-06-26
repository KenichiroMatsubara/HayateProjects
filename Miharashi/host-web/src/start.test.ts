import { describe, expect, it, vi } from 'vitest';
import {
  startMiharashiHost,
  type BootMiharashiHostOptions,
  type ReloadSubscription,
  type SubscribeReloadOptions,
} from './index.js';

/**
 * full reload ループの合成ルート（ホスト側）の配線契約テスト。初回 boot と、reload 受信ごとの
 * 再 boot を、boot / subscribe を注入 seam で差し替えて観測する。full reload は毎回**新しい
 * surface**で mount し直す（state は飛ぶ・ADR-0001 / CONTEXT.md「Reload」）。
 */

describe('startMiharashiHost', () => {
  it('boots once on start, against a freshly acquired canvas', () => {
    const boot = vi.fn(async (_o: BootMiharashiHostOptions) => {});
    const canvas = {} as HTMLCanvasElement;

    startMiharashiHost({
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
    const boot = vi.fn(async (_o: BootMiharashiHostOptions) => {});
    const canvases = [{ id: 1 } as unknown as HTMLCanvasElement, { id: 2 } as unknown as HTMLCanvasElement];
    const acquireCanvas = vi.fn(() => canvases[acquireCanvas.mock.calls.length - 1]!);
    let onReload: () => void = () => {};

    startMiharashiHost({
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

  it('closing the handle unsubscribes from reloads', () => {
    const subscription: ReloadSubscription = { close: vi.fn() };

    const handle = startMiharashiHost({
      devServerUrl: 'http://dev.example',
      hostProtocolVersion: 1,
      acquireCanvas: () => ({}) as HTMLCanvasElement,
      boot: async () => {},
      subscribe: () => subscription,
    });

    handle.close();

    expect(subscription.close).toHaveBeenCalledTimes(1);
  });
});
