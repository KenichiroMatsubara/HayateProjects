import { startMiharashiHost } from '@miharashi/host-web';

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

const params = new URLSearchParams(window.location.search);
const devServerUrl = params.get('dev') ?? DEFAULT_DEV_SERVER_URL;
const root = document.documentElement;

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
  acquireCanvas,
  onBootSettled: (result) => {
    if (result.ok) {
      mountCount += 1;
      root.dataset.miharashiMountCount = String(mountCount);
      root.dataset.miharashiStatus = 'mounted';
    } else {
      root.dataset.miharashiStatus = 'error';
      root.dataset.miharashiError = String(result.error);
      console.error('Miharashi host boot failed', result.error);
    }
  },
});
