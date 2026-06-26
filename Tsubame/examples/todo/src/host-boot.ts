import { startMiharashiHost, ProtocolMismatchError } from '@miharashi/host-web';
import { HOST_PROTOCOL_VERSION } from '@hayate/host';

/**
 * Miharashi ホストページのブート（ADR-0001）。host bootstrap だけを持つ素のシェルとして、
 * dev-server URL から App Bundle を fetch → eval し、canvas 上に `createHayateWebHost` で
 * raw + frame-clock を確立してバンドルの mount に渡す。さらに dev-server の reload WS を購読し、
 * バンドル変更ごとに **full reload**（新しい canvas で再 mount。state は飛ぶ）する。
 *
 * フレームワークも `@tsubame/renderer-canvas` もここには無い — eval するバンドルが持ち込む。
 * reload の意味づけはホスト側（`@miharashi/host-web`）に閉じ、ネイティブ契約は不変（ADR-0001）。
 */

/** `?dev=` 未指定時の dev-server origin。e2e / ローカルの既定ポートに合わせる。 */
const DEFAULT_DEV_SERVER_URL = 'http://localhost:5181';

/** host.html の surface canvas の id。full reload で差し替える新 canvas にも同じ id を付ける。 */
const CANVAS_ID = 'miharashi-canvas';

/**
 * このホスト（decoder）の protocol version を上書きする dev-only クエリ。既定は焼き込みの
 * {@link HOST_PROTOCOL_VERSION}。e2e が protocol 不一致ケース（ホスト版数 ≠ バンドル版数）を
 * 再現するための seam（#530）。
 */
const HOST_PROTOCOL_VERSION_OVERRIDE_PARAM = 'protocolVersion';

/** protocol 不一致の明示エラー UI を載せる要素の id。e2e はこれの可視と本文を検証する。 */
const ERROR_PANEL_ID = 'miharashi-error';

const params = new URLSearchParams(window.location.search);
const devServerUrl = params.get('dev') ?? DEFAULT_DEV_SERVER_URL;
const root = document.documentElement;

// e2e の不一致再現用。未指定なら焼き込みの decoder 版数を使う。
const overrideRaw = params.get(HOST_PROTOCOL_VERSION_OVERRIDE_PARAM);
const hostProtocolVersion =
  overrideRaw != null && overrideRaw !== '' ? Number(overrideRaw) : HOST_PROTOCOL_VERSION;

/**
 * protocol 不一致の明示エラー UI を表示する。謎クラッシュにせず「このホストは protocol vX、
 * バンドルは vY」を画面に出す（#530）。canvas は mount しないまま残す。
 */
function showProtocolMismatch(error: ProtocolMismatchError): void {
  root.dataset.miharashiStatus = 'protocol-mismatch';
  let panel = document.getElementById(ERROR_PANEL_ID);
  if (!panel) {
    panel = document.createElement('div');
    panel.id = ERROR_PANEL_ID;
    panel.setAttribute('role', 'alert');
    panel.style.cssText =
      'position:fixed;inset:0;display:flex;align-items:center;justify-content:center;' +
      'padding:24px;color:#fca5a5;background:#0b1020;font:16px/1.6 system-ui,sans-serif;text-align:center;';
    document.body.appendChild(panel);
  }
  panel.textContent = `protocol version 不一致: ${error.message}`;
}

// e2e / デバッグが「何回 mount まで貫けたか」を観測できるよう mount 回数を data 属性に出す。
// full reload が効くと、ソース編集のたびにこの数が増える。
let mountCount = 0;

/**
 * full reload 用に新しい surface を用意する。古い canvas を捨て、同 id・同スタイルの新品を
 * 差し込んで返す。canvas のコンテキスト型は一度決まると変えられないため、再 mount には
 * 新しい canvas が要る（既存 canvas への再 init は避ける）。
 *
 * 既知の制約：旧ホストの frame-clock / WASM レンダラは明示破棄せず、デタッチした canvas 上で
 * 放置される（full reload は state を捨てる前提の dev-client 挙動）。明示的な teardown は HMR
 * スライスの課題（ADR-0001：ホスト契約は full reload / HMR で不変）。
 */
function acquireCanvas(): HTMLCanvasElement {
  document.getElementById(CANVAS_ID)?.remove();
  const canvas = document.createElement('canvas');
  canvas.id = CANVAS_ID;
  document.body.appendChild(canvas);
  return canvas;
}

startMiharashiHost({
  devServerUrl,
  hostProtocolVersion,
  acquireCanvas,
  onBootSettled: (result) => {
    if (result.ok) {
      mountCount += 1;
      root.dataset.miharashiMountCount = String(mountCount);
      root.dataset.miharashiStatus = 'mounted';
    } else if (result.error instanceof ProtocolMismatchError) {
      // 不一致は mount もクラッシュもさせず、明示エラー UI に落とす（#530）。
      showProtocolMismatch(result.error);
      console.error('Miharashi host protocol mismatch', result.error);
    } else {
      root.dataset.miharashiStatus = 'error';
      root.dataset.miharashiError = String(result.error);
      console.error('Miharashi host boot failed', result.error);
    }
  },
});
