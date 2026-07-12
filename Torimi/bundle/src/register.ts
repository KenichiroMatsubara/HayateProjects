import type { WebHost } from '@torimi/hayate-host';
import { createHayateNativeHost, type RawHayate } from '@torimi/hayate-host/native';
import { TORIMI_PROTOCOL_VERSION_GLOBAL } from '@torimi/protocol-handshake';
import { runTsubameApp, type Host, type TsubameMount } from '@torimi/tsubame-app';
import { HayateRenderer, PROTOCOL_VERSION } from '@torimi/tsubame-renderer-hayate';

/**
 * App Bundle が mount を露出する global プロパティ名。`@torimi/host-web` の
 * `TORIMI_MOUNT_GLOBAL` と一致させる wire 契約（等値は wire-contract.test.ts が固定する）。
 * host-web からの import にしないのは依存方向のため — バンドル側パッケージがホスト実装に
 * 依存すると、ホストのコード（fetch/eval/reload ループ）が App Bundle に紛れ込む。
 */
export const TORIMI_MOUNT_GLOBAL = '__torimiMount';

/**
 * ホストから渡される host bootstrap のうち、バンドル側が結線に使う面。Web の
 * {@link WebHost} と native の `NativeHost` の共通部分（raw + frame-clock）で、
 * `HayateRenderer` の構築入力そのもの。
 */
type HostBootstrap = Pick<WebHost, 'raw' | 'requestFrame' | 'cancelFrame'>;

/**
 * 押し込まれた host bootstrap（raw + frame-clock）を `Host` port に包み、バンドルが持ち込む
 * host-blind `HayateRenderer` を構築して合成ルートへ渡す（ADR-0012 の薄い対称合成）。
 */
function mountWithBootstrap(bootstrap: HostBootstrap, mount: TsubameMount): () => void {
  let renderer: HayateRenderer | undefined;
  const host: Host = {
    createRenderer() {
      renderer = new HayateRenderer({
        raw: bootstrap.raw,
        requestFrame: bootstrap.requestFrame,
        cancelFrame: bootstrap.cancelFrame,
      });
      renderer.start();
      return renderer;
    },
    stop: () => renderer?.stop(),
  };
  return runTsubameApp(host, mount);
}

/**
 * App Bundle 側の合成ルート（Bundle Registration, ADR-0008 §4 / CONTEXT.md）。アプリは
 * 全ターゲット共通の 1 エントリでこれを呼ぶだけ — protocol version の焼き込み・mount seam
 * （`__torimiMount` / `__tsubame`）の登録といった wire 契約の配線はここが隠す。FW 知識は
 * {@link TsubameMount} 引数として受けるのみ（FW 盲目）。
 *
 * ターゲット差は `__hayateHost` の有無によるランタイム判定で内部分岐する：
 *
 * - **Native Host target**（`__hayateHost` あり）: ネイティブが JSI で注入した raw を
 *   `createHayateNativeHost` の pump 型 frame-clock に結線して即 mount し、ネイティブ vsync
 *   ループ用の `__tsubame`（pumpFrame / stop）を露出する（ADR-0112）。
 * - **Web Host target**（`__hayateHost` なし）: `__torimiMount` を登録する。ホストが eval 後に
 *   host bootstrap を渡して呼び、その時点で mount する。
 */
export function registerTorimiApp(mount: TsubameMount): void {
  const g = globalThis as Record<string, unknown>;

  // 内包する `@torimi/tsubame-renderer-hayate` の wire 定数バージョンを protocol version として
  // 焼き込む（#530 / #533）。ホスト（Web/Native）は eval 後にこれを読み、自身の decoder
  // 版数と突き合わせて一致時のみ mount する。
  g[TORIMI_PROTOCOL_VERSION_GLOBAL] = PROTOCOL_VERSION;

  const injectedRaw = g.__hayateHost as RawHayate | undefined;
  if (injectedRaw !== undefined) {
    // Native: host（注入 raw + vsync pump）は WASM をロードしない — Hayate はネイティブ
    // cdylib として既に存在する。フレーム駆動はネイティブ vsync が `__tsubame.pumpFrame` で
    // 1 フレームずつ行う。
    const nativeHost = createHayateNativeHost(injectedRaw);
    const dispose = mountWithBootstrap(nativeHost, mount);
    g.__tsubame = {
      pumpFrame: (timestampMs: number) => nativeHost.pumpFrame(timestampMs),
      stop: dispose,
    };
    return;
  }

  g[TORIMI_MOUNT_GLOBAL] = (webHost: WebHost) => {
    mountWithBootstrap(webHost, mount);
  };
}
