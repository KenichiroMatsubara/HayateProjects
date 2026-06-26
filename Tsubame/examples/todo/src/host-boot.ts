import { bootMiharashiHost } from '@miharashi/host-web';

/**
 * Miharashi ホストページのブート（ADR-0001 のスライス #1）。host bootstrap だけを持つ
 * 素のシェルとして、dev-server URL から App Bundle を fetch → eval し、canvas 上に
 * `createHayateWebHost` で raw + frame-clock を確立してバンドルの mount に渡す。
 *
 * フレームワークも `@tsubame/renderer-canvas` もここには無い — eval するバンドルが持ち込む。
 */

/** `?dev=` 未指定時の dev-server origin。e2e / ローカルの既定ポートに合わせる。 */
const DEFAULT_DEV_SERVER_URL = 'http://localhost:5181';

const params = new URLSearchParams(window.location.search);
const devServerUrl = params.get('dev') ?? DEFAULT_DEV_SERVER_URL;
const canvas = document.getElementById('miharashi-canvas') as HTMLCanvasElement;
const root = document.documentElement;

// e2e / デバッグが「mount まで貫けたか」を観測できるよう状態を data 属性に出す。
bootMiharashiHost({ devServerUrl, canvas }).then(
  () => {
    root.dataset.miharashiStatus = 'mounted';
  },
  (err: unknown) => {
    root.dataset.miharashiStatus = 'error';
    root.dataset.miharashiError = String(err);
    console.error('Miharashi host boot failed', err);
  },
);
