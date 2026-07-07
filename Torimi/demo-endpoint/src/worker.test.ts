import {
  demoEndpointContract,
  devServerContract,
  type DemoManifest,
} from '@torimi/dev-server-contract';
import { SELF } from 'cloudflare:test';
import { describe, expect, it } from 'vitest';

/** テスト用の任意 origin。Worker はホスト名を解釈しない（パスだけ見る）。 */
const DEMO_ORIGIN = 'https://torimi-demo.test';

describe('demo endpoint worker', () => {
  it('serves a Demo Manifest that conforms to the wire contract at the manifest route', async () => {
    const res = await SELF.fetch(`${DEMO_ORIGIN}${demoEndpointContract.demoManifestRoute}`);

    expect(res.status).toBe(200);
    expect(res.headers.get('content-type')).toContain('application/json');

    const manifest = (await res.json()) as DemoManifest;
    // ホストのメニュー構成と初回自動ロード（先頭エントリ）が成立する非空一覧であること。
    expect(manifest.demos.length).toBeGreaterThan(0);
    for (const entry of manifest.demos) {
      // wire 型ぴったり：表示名とバンドル URL のみ（build metadata を漏らさない）。
      expect(Object.keys(entry).sort()).toEqual(['bundleUrl', 'name']);
      expect(typeof entry.name).toBe('string');
      expect(typeof entry.bundleUrl).toBe('string');
    }
  });

  it('serves each manifest bundleUrl as a JS static asset (host fetches exactly these URLs)', async () => {
    const manifestRes = await SELF.fetch(`${DEMO_ORIGIN}${demoEndpointContract.demoManifestRoute}`);
    const manifest = (await manifestRes.json()) as DemoManifest;

    for (const entry of manifest.demos) {
      const res = await SELF.fetch(`${DEMO_ORIGIN}${entry.bundleUrl}`);
      expect(res.status).toBe(200);
      expect(res.headers.get('content-type')).toContain('javascript');
    }
  });

  it('accepts a WS upgrade at the reload route and holds it silently (no reload, no close)', async () => {
    const res = await SELF.fetch(`${DEMO_ORIGIN}${devServerContract.reloadRoute}`, {
      headers: { upgrade: 'websocket' },
    });

    expect(res.status).toBe(101);
    const socket = res.webSocket;
    expect(socket).not.toBeNull();
    if (socket == null) return;

    socket.accept();
    const received: string[] = [];
    let closed = false;
    socket.addEventListener('message', (event) => received.push(String(event.data)));
    socket.addEventListener('close', () => {
      closed = true;
    });

    // ホストの 1 秒 backoff 再接続の受け皿：即切断せず、reload も送らないこと。
    await new Promise((resolve) => setTimeout(resolve, 100));
    expect(closed).toBe(false);
    expect(received).toEqual([]);
    socket.close();
  });
});
