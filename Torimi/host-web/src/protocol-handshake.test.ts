import { describe, expect, it, vi } from 'vitest';
import { ProtocolMismatchError } from '@torimi/protocol-handshake';
import { bootTorimiHost } from './index.js';
import type { WebHost } from '@torimi/hayate-host';

/**
 * Torimi 起動時の protocol version ハンドシェイク契約（#530）。バンドル（encoder）が埋めた
 * 版数とホスト（decoder）の版数を突き合わせ、一致時のみ mount し、不一致時は明示エラー
 * （{@link ProtocolMismatchError}）で mount もクラッシュもさせない。実 WASM / 実ネットワークは
 * 巻き込まず、fetch / eval / createHost / バンドル版数読み取りを注入 seam で観測する。
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

describe('bootTorimiHost — protocol version handshake', () => {
  it('版数が一致したら従来通り mount する', async () => {
    const mount = vi.fn();

    await bootTorimiHost({
      devServerUrl: 'http://dev.example',
      canvas,
      hostProtocolVersion: 1,
      fetchBundle: async () => 'src',
      evalBundle: () => mount,
      readBundleVersion: () => 1,
      createHost: async () => fakeHost(),
    });

    expect(mount).toHaveBeenCalledTimes(1);
  });

  it('版数が不一致なら ProtocolMismatchError を投げ、mount しない', async () => {
    const mount = vi.fn();

    await expect(
      bootTorimiHost({
        devServerUrl: 'http://dev.example',
        canvas,
        hostProtocolVersion: 1,
        fetchBundle: async () => 'src',
        evalBundle: () => mount,
        readBundleVersion: () => 2,
        createHost: async () => fakeHost(),
      }),
    ).rejects.toBeInstanceOf(ProtocolMismatchError);

    expect(mount).not.toHaveBeenCalled();
  });

  it('不一致では host bootstrap すら確立しない（WASM/surface を無駄に起こさない）', async () => {
    const createHost = vi.fn(async () => fakeHost());

    await expect(
      bootTorimiHost({
        devServerUrl: 'http://dev.example',
        canvas,
        hostProtocolVersion: 1,
        fetchBundle: async () => 'src',
        evalBundle: () => vi.fn(),
        readBundleVersion: () => 9,
        createHost,
      }),
    ).rejects.toBeInstanceOf(ProtocolMismatchError);

    expect(createHost).not.toHaveBeenCalled();
  });

  it('不一致エラーは両バージョンを構造化して運ぶ（明示 UI 用）', async () => {
    const error = await bootTorimiHost({
      devServerUrl: 'http://dev.example',
      canvas,
      hostProtocolVersion: 1,
      fetchBundle: async () => 'src',
      evalBundle: () => vi.fn(),
      readBundleVersion: () => 2,
      createHost: async () => fakeHost(),
    }).catch((e: unknown) => e);

    expect(error).toBeInstanceOf(ProtocolMismatchError);
    const mismatch = error as ProtocolMismatchError;
    expect(mismatch.hostVersion).toBe(1);
    expect(mismatch.bundleVersion).toBe(2);
  });
});
