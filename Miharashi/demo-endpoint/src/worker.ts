/**
 * Miharashi Demo Endpoint（ADR-0003）。ビルド済みデモ App Bundle（静的アセット）と
 * Demo Manifest を常時 HTTPS 配信する。Dev Server と違い watch もビルドもせず、reload も
 * 送らない。
 */
import { demoEndpointContract, devServerContract } from '@miharashi/dev-server-contract';
import { demoManifest } from './demos.js';

export interface Env {
  /** wrangler の assets binding。デモバンドル（public/）の配信を委譲する先。 */
  readonly ASSETS: Fetcher;
}

/** Demo Manifest 応答の content-type。 */
const MANIFEST_CONTENT_TYPE = 'application/json; charset=utf-8';

/** WS ハンドシェイク成立のステータス（RFC 6455 / Workers の webSocket 応答）。 */
const HTTP_SWITCHING_PROTOCOLS = 101;

/** upgrade 要求ヘッダの WS 識別値。 */
const WEBSOCKET_UPGRADE = 'websocket';

/**
 * reload ルートへの WS 接続を受理して黙って保持する。Demo Endpoint は reload を送らない
 * （配信物はリリースと lockstep・ADR-0003）が、ホストの 1 秒 backoff 再接続が無意味な
 * 通信を打ち続けないよう、接続の受け皿にはなる。
 */
function holdReloadSocket(): Response {
  const pair = new WebSocketPair();
  const [client, server] = [pair[0], pair[1]];
  server.accept();
  // メッセージは送らず、切断もしない。相手が閉じるまでただ保持する。
  return new Response(null, { status: HTTP_SWITCHING_PROTOCOLS, webSocket: client });
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const { pathname } = new URL(request.url);

    if (pathname === demoEndpointContract.demoManifestRoute) {
      return new Response(JSON.stringify(demoManifest), {
        headers: { 'content-type': MANIFEST_CONTENT_TYPE },
      });
    }

    if (
      pathname === devServerContract.reloadRoute &&
      request.headers.get('upgrade')?.toLowerCase() === WEBSOCKET_UPGRADE
    ) {
      return holdReloadSocket();
    }

    // Demo Manifest / reload 以外はすべて静的アセット（デモバンドル）へ fallthrough。
    return env.ASSETS.fetch(request);
  },
} satisfies ExportedHandler<Env>;
