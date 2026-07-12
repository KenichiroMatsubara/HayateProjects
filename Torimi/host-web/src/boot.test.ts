import { afterEach, describe, expect, it, vi } from 'vitest';
import { bootTorimiHost, TORIMI_MOUNT_GLOBAL } from './index.js';
import type { WebHost } from '@torimi/hayate-host';

/**
 * Torimi Web ホストの配線契約テスト。実 WASM / 実ブラウザ / 実ネットワークを巻き込まず、
 * fetch / eval / createHayateWebHost を注入 seam で差し替え、「dev-server URL からバンドルを
 * 取得 → eval → host bootstrap を確立 → バンドルの mount に渡す」順序と受け渡しを観測する
 * （ADR-0001。@torimi/hayate-host の web-host.test.ts と同型）。
 */
function fakeHost(): WebHost {
  return {
    raw: {} as WebHost['raw'],
    requestFrame: () => 0,
    cancelFrame: () => undefined,
    detach: () => undefined,
  };
}

const canvas = {} as HTMLCanvasElement;

describe('bootTorimiHost', () => {
  afterEach(() => {
    delete (globalThis as Record<string, unknown>)[TORIMI_MOUNT_GLOBAL];
  });

  it('mounts the fetched+evaled bundle with the host bootstrap', async () => {
    const host = fakeHost();
    const mount = vi.fn();

    await bootTorimiHost({
      devServerUrl: 'http://dev.example',
      canvas,
      hostProtocolVersion: 1,
      fetchBundle: async () => 'BUNDLE_SOURCE',
      evalBundle: () => mount,
      readBundleVersion: () => 1,
      createHost: async () => host,
    });

    expect(mount).toHaveBeenCalledWith(host);
  });

  it('fetches the bundle from the dev-server URL at the bundle route', async () => {
    const fetchBundle = vi.fn(async () => 'src');

    await bootTorimiHost({
      devServerUrl: 'http://127.0.0.1:5179',
      canvas,
      hostProtocolVersion: 1,
      fetchBundle,
      evalBundle: () => vi.fn(),
      readBundleVersion: () => 1,
      createHost: async () => fakeHost(),
    });

    expect(fetchBundle).toHaveBeenCalledWith('http://127.0.0.1:5179/bundle.js');
  });

  it('evals the bundle before establishing the host bootstrap', async () => {
    // 順序契約（ADR-0001）: fetch → eval → createHayateWebHost → mount。host を作る前に
    // バンドルを eval する（createHayateWebHost が surface/WASM を確立する前にバンドルを評価）。
    const calls: string[] = [];

    await bootTorimiHost({
      devServerUrl: 'http://dev.example',
      canvas,
      hostProtocolVersion: 1,
      fetchBundle: async () => 'src',
      evalBundle: () => {
        calls.push('eval');
        return vi.fn();
      },
      readBundleVersion: () => 1,
      createHost: async () => {
        calls.push('createHost');
        return fakeHost();
      },
    });

    expect(calls).toEqual(['eval', 'createHost']);
  });

  it('default eval picks up the bundle-registered global mount', async () => {
    const host = fakeHost();
    let received: WebHost | undefined;
    // 実バンドルが立てる global mount を模した最小ソース（IIFE 相当の副作用）。
    (globalThis as Record<string, unknown>).__torimiReceived = (h: WebHost) => {
      received = h;
    };
    const source = `globalThis.${TORIMI_MOUNT_GLOBAL} = (h) => globalThis.__torimiReceived(h);`;

    await bootTorimiHost({
      devServerUrl: 'http://dev.example',
      canvas,
      hostProtocolVersion: 1,
      fetchBundle: async () => source,
      readBundleVersion: () => 1,
      createHost: async () => host,
    });

    expect(received).toBe(host);
    delete (globalThis as Record<string, unknown>).__torimiReceived;
  });

  it('rejects a bundle that does not register the mount global', async () => {
    await expect(
      bootTorimiHost({
        devServerUrl: 'http://dev.example',
        canvas,
        hostProtocolVersion: 1,
        fetchBundle: async () => '/* no mount registered */',
        createHost: async () => fakeHost(),
      }),
    ).rejects.toThrow(TORIMI_MOUNT_GLOBAL);
  });
});
