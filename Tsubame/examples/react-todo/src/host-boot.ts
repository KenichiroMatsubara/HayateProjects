import {
  isCameraScanSupported,
  scanQrFromCamera,
  startMiharashiHost,
  ProtocolMismatchError,
} from '@miharashi/host-web';
import { HOST_PROTOCOL_VERSION } from '@hayate/host';

/**
 * Miharashi ホストページのブート（ADR-0001）。host bootstrap だけを持つ素のシェルとして、
 * dev-server URL から App Bundle を fetch → eval し、canvas 上に `createHayateWebHost` で
 * raw + frame-clock を確立してバンドルの mount に渡す。さらに dev-server の reload WS を購読し、
 * バンドル変更ごとに **full reload**（新しい canvas で再 mount。state は飛ぶ）する。
 *
 * フレームワークも `@tsubame/renderer-hayate` もここには無い — eval するバンドルが持ち込む。
 * reload の意味づけはホスト側（`@miharashi/host-web`）に閉じ、ネイティブ契約は不変（ADR-0001）。
 *
 * dev-server URL は `?dev=` で渡せるが、未指定ならスマホで使える URL ピッカー（カメラ QR スキャン
 * ＋手入力）を出す。起動コマンドが端末に出した LAN URL の QR をスマホのカメラで読めば、`?dev=` を
 * 手打ちせずに接続できる（CONTEXT.md「Dev Server」）。
 */

/** `?dev=` も保存値も無いときの dev-server origin。e2e / ローカルの既定ポートに合わせる。 */
const DEFAULT_DEV_SERVER_URL = 'http://localhost:5181';

/** 直近に使った dev-server URL を覚えておく localStorage キー（次回起動の手間を省く）。 */
const LAST_URL_STORAGE_KEY = 'miharashi.devServerUrl';

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

/** URL ピッカー（カメラ QR スキャン + 手入力）の root id。 */
const PICKER_ID = 'miharashi-picker';

const params = new URLSearchParams(window.location.search);
const root = document.documentElement;

// e2e の不一致再現用。未指定なら焼き込みの decoder 版数を使う。
const overrideRaw = params.get(HOST_PROTOCOL_VERSION_OVERRIDE_PARAM);
const hostProtocolVersion =
  overrideRaw != null && overrideRaw !== '' ? Number(overrideRaw) : HOST_PROTOCOL_VERSION;

/** `192.168.1.5:5179` のようにスキーム無しで入れても繋がるよう `http://` を補う。 */
function normalizeDevServerUrl(raw: string): string {
  const trimmed = raw.trim();
  return /^https?:\/\//.test(trimmed) ? trimmed : `http://${trimmed}`;
}

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

/**
 * dev-server URL のピッカーを出して、確定した URL で resolve する。カメラ QR スキャン
 * （対応ブラウザ）と手入力の両方を出す。起動コマンドの QR（= LAN URL）をカメラで読めば
 * そのまま接続できる。`?dev=` があるときは呼ばれない。
 */
function pickDevServerUrl(): Promise<string> {
  return new Promise((resolve) => {
    const saved = (() => {
      try {
        return window.localStorage.getItem(LAST_URL_STORAGE_KEY);
      } catch {
        return null;
      }
    })();

    const overlay = document.createElement('div');
    overlay.id = PICKER_ID;
    overlay.style.cssText =
      'position:fixed;inset:0;display:flex;flex-direction:column;gap:16px;align-items:center;' +
      'justify-content:center;padding:24px;background:#0b1020;color:#e2e8f0;' +
      'font:16px/1.6 system-ui,sans-serif;';

    const title = document.createElement('div');
    title.textContent = 'Miharashi に接続';
    title.style.cssText = 'font-size:20px;font-weight:600;';

    const hint = document.createElement('div');
    hint.textContent = '起動コマンドが表示した QR をカメラで読み取るか、dev-server URL を入力してください。';
    hint.style.cssText = 'max-width:32em;text-align:center;color:#94a3b8;';

    const input = document.createElement('input');
    input.type = 'text';
    input.inputMode = 'url';
    input.placeholder = 'dev-server URL（例 192.168.1.5:5181）';
    input.value = saved ?? '';
    input.style.cssText =
      'width:min(90vw,28em);padding:12px 14px;border-radius:8px;border:1px solid #334155;' +
      'background:#111827;color:#e2e8f0;font-size:16px;';

    const status = document.createElement('div');
    status.style.cssText = 'min-height:1.5em;color:#94a3b8;text-align:center;';

    // 背面カメラのプレビュー（スキャン中だけ表示）。
    const video = document.createElement('video');
    video.setAttribute('playsinline', '');
    video.muted = true;
    video.style.cssText =
      'display:none;width:min(90vw,28em);aspect-ratio:1/1;object-fit:cover;border-radius:12px;background:#000;';

    const buttons = document.createElement('div');
    buttons.style.cssText = 'display:flex;gap:12px;flex-wrap:wrap;justify-content:center;';

    const connect = document.createElement('button');
    connect.textContent = '接続';
    const buttonStyle =
      'padding:12px 18px;border-radius:8px;border:0;font-size:16px;cursor:pointer;' +
      'background:#2563eb;color:#fff;';
    connect.style.cssText = buttonStyle;

    let scanController: { cancel(): void } | undefined;

    const finish = (raw: string): void => {
      scanController?.cancel();
      const url = normalizeDevServerUrl(raw);
      try {
        window.localStorage.setItem(LAST_URL_STORAGE_KEY, raw.trim());
      } catch {
        // localStorage 不可（プライベートモード等）でも接続自体は続行する。
      }
      overlay.remove();
      resolve(url);
    };

    connect.addEventListener('click', () => {
      if (input.value.trim() === '') {
        status.textContent = 'URL を入力してください。';
        return;
      }
      finish(input.value);
    });
    input.addEventListener('keydown', (ev) => {
      if (ev.key === 'Enter' && input.value.trim() !== '') finish(input.value);
    });

    buttons.appendChild(connect);

    if (isCameraScanSupported()) {
      const scan = document.createElement('button');
      scan.textContent = 'QR スキャン';
      scan.style.cssText = buttonStyle.replace('#2563eb', '#0ea5e9');
      scan.addEventListener('click', () => {
        if (scanController != null) return; // 二重起動を防ぐ
        status.textContent = 'カメラを起動しています…';
        video.style.display = 'block';
        scanController = scanQrFromCamera({
          video,
          onResult: (text) => {
            input.value = text;
            status.textContent = '読み取りました。接続します…';
            finish(text);
          },
          onError: (error) => {
            scanController = undefined;
            video.style.display = 'none';
            status.textContent = `カメラを使えませんでした（${String(error)}）。URL を入力してください。`;
          },
        });
      });
      buttons.appendChild(scan);
    } else {
      hint.textContent += '（このブラウザはカメラ QR 非対応です。URL を入力してください。）';
    }

    overlay.append(title, hint, video, input, buttons, status);
    document.body.appendChild(overlay);
    input.focus();
  });
}

/** ホストを起動する。reload 購読まで張る（full reload ループ）。 */
function boot(devServerUrl: string): void {
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
}

const devParam = params.get('dev');
if (devParam != null && devParam !== '') {
  // 明示指定（e2e / ブックマーク）はそのまま起動する。
  boot(devParam);
} else if (params.has('dev')) {
  // `?dev=`（空）は「既定で繋ぐ」意図とみなし、ピッカーを出さず従来の既定 URL で起動する。
  boot(DEFAULT_DEV_SERVER_URL);
} else {
  // 未指定ならスマホ向けピッカー（QR スキャン + 手入力）を出してから起動する。
  void pickDevServerUrl().then(boot);
}
