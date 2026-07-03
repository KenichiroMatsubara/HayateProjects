import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { WebHost } from '@hayate/host';
import { createBundleDevServer, type BundleDevServer } from '@miharashi/dev-server';
import {
  MIHARASHI_MOUNT_GLOBAL,
  bootMiharashiHost,
  subscribeReload,
  type ReloadSubscription,
} from '@miharashi/host-web';
import { MIHARASHI_PROTOCOL_VERSION_GLOBAL } from '@miharashi/protocol-handshake';

/**
 * `@miharashi/dev-server` と `@miharashi/host-web` を実ネットワーク経由で繋いで通しで検証する。
 * どちらのパッケージも自分の半分（`devServerContract` を守っているか）は個別にテスト済みだが、
 * 両者を実際に組み合わせて「fetch → eval → protocol handshake → mount」「reload 通知」が
 * 本当に噛み合うかは、これまでどのテストも見ていなかった（候補 #2 の付随課題）。
 */
function fakeHost(): WebHost {
  return {
    raw: {} as WebHost['raw'],
    requestFrame: () => 0,
    cancelFrame: () => undefined,
    detach: () => undefined,
  };
}

describe('Dev Server ↔ Host wired together over a real network path', () => {
  let dir: string;
  let bundlePath: string;
  let server: BundleDevServer;
  let origin: string;
  let subscription: ReloadSubscription | undefined;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), 'miharashi-integration-'));
    bundlePath = join(dir, 'bundle.js');
  });

  afterEach(async () => {
    subscription?.close();
    subscription = undefined;
    await server?.close();
    await rm(dir, { recursive: true, force: true });
    delete (globalThis as Record<string, unknown>)[MIHARASHI_MOUNT_GLOBAL];
    delete (globalThis as Record<string, unknown>)[MIHARASHI_PROTOCOL_VERSION_GLOBAL];
  });

  it('boots by fetching the real App Bundle from a real dev-server over HTTP', async () => {
    await writeFile(
      bundlePath,
      `globalThis.${MIHARASHI_PROTOCOL_VERSION_GLOBAL} = 7;
       globalThis.${MIHARASHI_MOUNT_GLOBAL} = (host) => { globalThis.__integrationMountedWith = host; };`,
    );
    server = createBundleDevServer({ bundlePath });
    origin = await server.listen();

    const host = fakeHost();
    await bootMiharashiHost({
      devServerUrl: origin,
      canvas: {} as HTMLCanvasElement,
      hostProtocolVersion: 7,
      createHost: async () => host,
    });

    expect((globalThis as Record<string, unknown>).__integrationMountedWith).toBe(host);
    delete (globalThis as Record<string, unknown>).__integrationMountedWith;
  });

  it('rejects with ProtocolMismatchError when the served bundle is on a different protocol version', async () => {
    await writeFile(
      bundlePath,
      `globalThis.${MIHARASHI_PROTOCOL_VERSION_GLOBAL} = 1;
       globalThis.${MIHARASHI_MOUNT_GLOBAL} = () => {};`,
    );
    server = createBundleDevServer({ bundlePath });
    origin = await server.listen();

    await expect(
      bootMiharashiHost({
        devServerUrl: origin,
        canvas: {} as HTMLCanvasElement,
        hostProtocolVersion: 2,
        createHost: async () => fakeHost(),
      }),
    ).rejects.toThrow(/protocol/i);
  });

  it('delivers a real WebSocket reload broadcast to subscribeReload when the served bundle changes', async () => {
    await writeFile(bundlePath, 'globalThis.__miharashiMount = () => {};\n');
    server = createBundleDevServer({ bundlePath, debounceMs: 10 });
    origin = await server.listen();

    const onReload = new Promise<void>((resolve) => {
      subscription = subscribeReload({ devServerUrl: origin, onReload: resolve });
    });

    await writeFile(bundlePath, 'globalThis.__miharashiMount = () => { /* edited */ };\n');

    await expect(onReload).resolves.toBeUndefined();
  });
});
