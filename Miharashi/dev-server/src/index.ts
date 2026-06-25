import { readFile } from 'node:fs/promises';
import { createServer, type Server } from 'node:http';
import type { AddressInfo } from 'node:net';

/** App Bundle（単一 JS）を配信する HTTP ルート。ホスト側はこのパスで fetch する。 */
export const BUNDLE_ROUTE = '/bundle.js';

/** バンドル応答の content-type。ホストは text を fetch して eval するが、ブラウザが JS と
 * 解せるよう正しい MIME を返す。 */
const BUNDLE_CONTENT_TYPE = 'application/javascript; charset=utf-8';

/** CORS 許可 origin。ホストページは別 origin（dev 環境では別ポート）で動き fetch するので
 * 全許可にする。dev-only ツールであり認証情報も扱わない。 */
const ACCESS_CONTROL_ALLOW_ORIGIN = '*';

export interface BundleDevServerOptions {
  /** 配信する単一 App Bundle（JS）の絶対パス。 */
  readonly bundlePath: string;
  /** バインドするポート。既定 0（OS が空きポートを割り当てる）。 */
  readonly port?: number;
  /** バインドするホスト名。既定は loopback。 */
  readonly hostname?: string;
}

export interface BundleDevServer {
  /** listen し、解決後の origin（例 `http://127.0.0.1:5179`）を返す。 */
  listen(): Promise<string>;
  /** listen を解除する。 */
  close(): Promise<void>;
}

/** 既定 bind ホスト。loopback に固定し、dev server を外部公開しない。 */
const DEFAULT_HOSTNAME = '127.0.0.1';
/** 既定 bind ポート。0 は OS による空きポート割当。 */
const DEFAULT_PORT = 0;

class NodeBundleDevServer implements BundleDevServer {
  readonly #server: Server;
  readonly #port: number;
  readonly #hostname: string;

  constructor(options: BundleDevServerOptions) {
    this.#port = options.port ?? DEFAULT_PORT;
    this.#hostname = options.hostname ?? DEFAULT_HOSTNAME;
    this.#server = createServer((req, res) => {
      res.setHeader('access-control-allow-origin', ACCESS_CONTROL_ALLOW_ORIGIN);
      if (req.url === BUNDLE_ROUTE) {
        readFile(options.bundlePath).then(
          (body) => {
            res.statusCode = 200;
            res.setHeader('content-type', BUNDLE_CONTENT_TYPE);
            res.end(body);
          },
          () => {
            res.statusCode = 404;
            res.end();
          },
        );
        return;
      }
      res.statusCode = 404;
      res.end();
    });
  }

  listen(): Promise<string> {
    return new Promise((resolve) => {
      this.#server.listen(this.#port, this.#hostname, () => {
        const { port } = this.#server.address() as AddressInfo;
        resolve(`http://${this.#hostname}:${port}`);
      });
    });
  }

  close(): Promise<void> {
    return new Promise((resolve, reject) => {
      this.#server.close((err) => (err ? reject(err) : resolve()));
    });
  }
}

/**
 * Miharashi の最小 dev server を生成する。`bundlePath` の単一 App Bundle を
 * {@link BUNDLE_ROUTE} で HTTP 配信するだけ — watch / WS / protocol version は持たない
 * （後続スライス #2 / #3, ADR-0001）。
 */
export function createBundleDevServer(options: BundleDevServerOptions): BundleDevServer {
  return new NodeBundleDevServer(options);
}
