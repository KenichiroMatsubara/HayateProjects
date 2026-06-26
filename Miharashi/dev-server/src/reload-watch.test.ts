import { mkdtemp, rename, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { createBundleDevServer, RELOAD_MESSAGE, RELOAD_ROUTE, type BundleDevServer } from './index.js';

/**
 * full reload ループの dev-server 側契約テスト（ADR-0001 / CONTEXT.md「Reload」）。
 * 実際に listen し、本物の WebSocket（Node 22 の global WS クライアント）で `RELOAD_ROUTE` に
 * 繋ぎ、watch 対象ファイルを書き換えて「dev-server がソース変更を検知して WS で `reload` を
 * 送る」を本物のネットワーク経路で確認する。
 */

/** origin（`http://host:port`）を WS スキームに直す。 */
function toWs(origin: string): string {
  return origin.replace(/^http/, 'ws');
}

/** 次の WS テキストメッセージを待つ。 */
function nextMessage(ws: WebSocket): Promise<string> {
  return new Promise((resolve, reject) => {
    ws.addEventListener('message', (ev) => resolve(String((ev as MessageEvent).data)), { once: true });
    ws.addEventListener('error', () => reject(new Error('WS error')), { once: true });
  });
}

/** WS の open を待つ。 */
function opened(ws: WebSocket): Promise<void> {
  return new Promise((resolve, reject) => {
    ws.addEventListener('open', () => resolve(), { once: true });
    ws.addEventListener('error', () => reject(new Error('WS open failed')), { once: true });
  });
}

describe('createBundleDevServer — reload over WebSocket', () => {
  let dir: string;
  let bundlePath: string;
  let server: BundleDevServer;
  let origin: string;
  const sockets: WebSocket[] = [];

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), 'miharashi-reload-'));
    bundlePath = join(dir, 'bundle.js');
    await writeFile(bundlePath, 'globalThis.__miharashiMount = () => {};\n');
    // テストを速く確定的にするため debounce は小さく。
    server = createBundleDevServer({ bundlePath, debounceMs: 10 });
    origin = await server.listen();
  });

  afterEach(async () => {
    for (const ws of sockets) ws.close();
    sockets.length = 0;
    await server.close();
    await rm(dir, { recursive: true, force: true });
  });

  it('broadcasts a reload signal to connected clients when the watched bundle changes', async () => {
    const ws = new WebSocket(`${toWs(origin)}${RELOAD_ROUTE}`);
    sockets.push(ws);
    await opened(ws);

    const message = nextMessage(ws);
    await writeFile(bundlePath, 'globalThis.__miharashiMount = () => { /* edited */ };\n');

    expect(await message).toBe(RELOAD_MESSAGE);
  });

  it('coalesces a burst of changes into a single reload (debounce)', async () => {
    const ws = new WebSocket(`${toWs(origin)}${RELOAD_ROUTE}`);
    sockets.push(ws);
    await opened(ws);

    let reloadCount = 0;
    ws.addEventListener('message', (ev) => {
      if (String((ev as MessageEvent).data) === RELOAD_MESSAGE) reloadCount += 1;
    });

    // ビルドが 1 編集で起こす複数書き込みを模した連続変更。
    await writeFile(bundlePath, '/* a */\n');
    await writeFile(bundlePath, '/* b */\n');
    await writeFile(bundlePath, '/* c */\n');

    // debounce(10ms) を十分に超えて待つ。1 回だけ届くこと。
    await new Promise((r) => setTimeout(r, 120));
    expect(reloadCount).toBe(1);
  });

  it('detects an atomic replace (write temp + rename) as a change', async () => {
    // ビルドツールは出力をアトミックに差し替える（temp に書いて rename）ことがある。ファイル単体で
    // なく親ディレクトリを watch するのは、この rename を取りこぼさないため。
    const ws = new WebSocket(`${toWs(origin)}${RELOAD_ROUTE}`);
    sockets.push(ws);
    await opened(ws);

    const message = nextMessage(ws);
    const tmp = `${bundlePath}.tmp`;
    await writeFile(tmp, 'globalThis.__miharashiMount = () => { /* rebuilt */ };\n');
    await rename(tmp, bundlePath);

    expect(await message).toBe(RELOAD_MESSAGE);
  });
});
