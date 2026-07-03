import { createHash } from 'node:crypto';
import { mkdirSync, watch, type FSWatcher } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { createServer, type IncomingMessage, type Server } from 'node:http';
import type { AddressInfo, Socket } from 'node:net';
import { basename, dirname } from 'node:path';
import { devServerContract } from '@miharashi/dev-server-contract';

export { ALL_INTERFACES_HOSTNAME, localNetworkUrls, type LocalNetworkUrl } from './network.js';
export { encodeQr, qrToTerminalString, type QrMatrix, type QrTerminalOptions } from './qr.js';
export {
  buildStartupBanner,
  printStartupBanner,
  type StartupBannerOptions,
} from './startup-banner.js';

/**
 * watch のデバウンス時間（ms）。ビルドは 1 編集で複数のファイル書き込みを起こすため、
 * 連続イベントを 1 回の reload にまとめる。**プレースホルダ値**（実値調整は #8, ADR-0001）。
 */
export const WATCH_DEBOUNCE_MS = 80;

/** WebSocket ハンドシェイクの magic GUID（RFC 6455）。Sec-WebSocket-Accept の算出に使う。 */
const WS_ACCEPT_GUID = '258EAFA5-E914-47DA-95CA-C5AB0DC85B11';

/** WS テキストフレームの先頭バイト：FIN=1・opcode=0x1（text）。 */
const WS_TEXT_FRAME_FIN_TEXT = 0x81;

/** マスク無しペイロードで拡張長を使わない上限（RFC 6455）。reload は十分短いのでこの範囲に収まる。 */
const WS_MAX_SINGLE_BYTE_PAYLOAD = 125;

/** `Sec-WebSocket-Key` から `Sec-WebSocket-Accept` を算出する（RFC 6455 ハンドシェイク）。 */
function computeAcceptKey(secWebSocketKey: string): string {
  return createHash('sha1')
    .update(secWebSocketKey + WS_ACCEPT_GUID)
    .digest('base64');
}

/** サーバ→クライアントのマスク無しテキストフレームへエンコードする（短い制御メッセージ専用）。 */
function encodeTextFrame(text: string): Buffer {
  const payload = Buffer.from(text, 'utf8');
  if (payload.length > WS_MAX_SINGLE_BYTE_PAYLOAD) {
    // reload 等の短い制御メッセージしか送らない前提。拡張長は未実装。
    throw new Error('Miharashi dev-server: WS payload too large for single-byte length frame');
  }
  return Buffer.concat([Buffer.from([WS_TEXT_FRAME_FIN_TEXT, payload.length]), payload]);
}

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
  /**
   * 変更を監視するパス。変更を検知したら接続中のホストに {@link devServerContract} の reloadMessage を送る。
   * 既定は {@link bundlePath}（配信している成果物そのものを監視する）。ビルドは外部
   * （例 `vite build --watch`）が担い、dev-server は成果物の更新を見て reload を中継するだけ
   * — FW/ビルドツール非依存を保つ（ADR-0001）。
   */
  readonly watchPath?: string;
  /** watch のデバウンス時間（ms）。既定は {@link WATCH_DEBOUNCE_MS}。 */
  readonly debounceMs?: number;
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
  readonly #watchPath: string;
  readonly #debounceMs: number;
  /** reload を待つ接続中ホストの WS ソケット群。 */
  readonly #clients = new Set<Socket>();
  #watcher: FSWatcher | undefined;
  #debounceTimer: ReturnType<typeof setTimeout> | undefined;

  constructor(options: BundleDevServerOptions) {
    this.#port = options.port ?? DEFAULT_PORT;
    this.#hostname = options.hostname ?? DEFAULT_HOSTNAME;
    this.#watchPath = options.watchPath ?? options.bundlePath;
    this.#debounceMs = options.debounceMs ?? WATCH_DEBOUNCE_MS;
    this.#server = createServer((req, res) => {
      res.setHeader('access-control-allow-origin', ACCESS_CONTROL_ALLOW_ORIGIN);
      if (req.url === devServerContract.bundleRoute) {
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
    this.#server.on('upgrade', (req, socket) => this.#handleUpgrade(req, socket as Socket));
  }

  /** devServerContract.reloadRoute への WS ハンドシェイクを受理し、ソケットを reload 配信対象に加える。 */
  #handleUpgrade(req: IncomingMessage, socket: Socket): void {
    const key = req.headers['sec-websocket-key'];
    if (req.url !== devServerContract.reloadRoute || typeof key !== 'string') {
      socket.destroy();
      return;
    }
    socket.write(
      'HTTP/1.1 101 Switching Protocols\r\n' +
        'Upgrade: websocket\r\n' +
        'Connection: Upgrade\r\n' +
        `Sec-WebSocket-Accept: ${computeAcceptKey(key)}\r\n` +
        '\r\n',
    );
    this.#clients.add(socket);
    const drop = (): void => {
      this.#clients.delete(socket);
    };
    socket.on('close', drop);
    socket.on('error', drop);
    // ホストは listen するだけなので受信フレームは読まない。close/error の検知だけ行う。
  }

  /** watch を開始する。最初の listen 後に呼ぶ。 */
  #startWatching(): void {
    // 成果物のアトミック書き換え（unlink+rename）でも取りこぼさないよう親ディレクトリを監視し、
    // 対象ファイル名のイベントだけ拾う。
    const targetName = basename(this.#watchPath);
    const watchDir = dirname(this.#watchPath);
    // ビルド（`vite build --watch`）が出力ディレクトリを作る前に listen することがある。
    // 監視対象の親ディレクトリを先に用意して、`fs.watch` の ENOENT を避ける（既存なら no-op）。
    mkdirSync(watchDir, { recursive: true });
    this.#watcher = watch(watchDir, (_event, filename) => {
      if (filename != null && filename !== targetName) return;
      this.#scheduleReload();
    });
  }

  /** デバウンスして reload をブロードキャストする。 */
  #scheduleReload(): void {
    if (this.#debounceTimer != null) clearTimeout(this.#debounceTimer);
    this.#debounceTimer = setTimeout(() => {
      this.#debounceTimer = undefined;
      this.#broadcastReload();
    }, this.#debounceMs);
  }

  /** 接続中の全ホストに {@link devServerContract} の reloadMessage を送る。 */
  #broadcastReload(): void {
    const frame = encodeTextFrame(devServerContract.reloadMessage);
    for (const socket of this.#clients) socket.write(frame);
  }

  listen(): Promise<string> {
    return new Promise((resolve) => {
      this.#server.listen(this.#port, this.#hostname, () => {
        this.#startWatching();
        const { port } = this.#server.address() as AddressInfo;
        resolve(`http://${this.#hostname}:${port}`);
      });
    });
  }

  close(): Promise<void> {
    if (this.#debounceTimer != null) clearTimeout(this.#debounceTimer);
    this.#watcher?.close();
    for (const socket of this.#clients) socket.destroy();
    this.#clients.clear();
    return new Promise((resolve, reject) => {
      this.#server.close((err) => (err ? reject(err) : resolve()));
    });
  }
}

/**
 * Miharashi の最小 dev server を生成する。`bundlePath` の単一 App Bundle を
 * {@link devServerContract} の bundleRoute で HTTP 配信するだけ — watch / WS / protocol version は持たない
 * （後続スライス #2 / #3, ADR-0001）。
 */
export function createBundleDevServer(options: BundleDevServerOptions): BundleDevServer {
  return new NodeBundleDevServer(options);
}
