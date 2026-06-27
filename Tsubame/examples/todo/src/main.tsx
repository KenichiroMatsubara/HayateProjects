import { DomRenderer } from '@tsubame/renderer-dom';
import { HayateRenderer } from '@tsubame/renderer-hayate';
import { renderTsubame } from '@tsubame/solid';
import {
  runTsubameApp,
  detectModeFromSearch,
  type DetectModeResult,
  type Host,
} from '@tsubame/app';
import { TodoApp } from './App';

function detectModeFromWindow(): DetectModeResult {
  return detectModeFromSearch(window.location.search, {
    hasEditContext: 'EditContext' in window,
    hasWebGPU: 'gpu' in navigator,
  });
}

const detected = detectModeFromWindow();
const dom = document.getElementById('dom-host') as HTMLDivElement;
const canvas = document.getElementById('canvas-stage') as HTMLCanvasElement;

// target（DOM / Hayate）の選択は Host に局在する。合成ルート `runTsubameApp` は IRenderer
// しか知らず、DomRenderer / HayateRenderer も WebGPU プローブも見ない（ADR-0012）。
// NOTE: 下の web Host adapter は react-todo の main.tsx と同型になる見込み。2 つ目が出た
// 時点で中立 App 階層パッケージへ抽出する（ADR-0012「1 adapter は仮の seam、2 で本物」）。
let hayateRenderer: HayateRenderer | undefined;
const host: Host =
  detected.mode === 'DOM'
    ? {
        // DOM 経路：Hayate を迂回し、native IME と CSS リフローに委ねる。viewport 追従は
        // ブラウザの CSS / `@media` が担い、Tsubame は resize を配線しない（ADR-0080）。
        createRenderer() {
          dom.hidden = false;
          return new DomRenderer({ container: dom });
        },
      }
    : {
        // Hayate 経路：host bootstrap は Hayate 側（`@hayate/host`）が持つ。host が WebGPU を
        // プローブし backend を選び WASM をロードして surface 上に raw を確立し、frame-clock も
        // 供給する。App は host から raw(+clock) を得て host-blind HayateRenderer に結線するだけ。
        async createRenderer() {
          const { createHayateWebHost } = await import('@hayate/host');
          canvas.hidden = false;
          // Dev-only: 配信ルートの手書き `tuning.jsonc` を拾い、F5 だけで感触定数を較正できる
          // （WASM 再ビルド不要）。404 / parse 失敗はコンパイル既定のまま。
          const tuning = await fetch(new URL('tuning.jsonc', document.baseURI).href)
            .then((r) => (r.ok ? r.text() : undefined))
            .catch(() => undefined);
          const webHost = await createHayateWebHost(canvas, {
            backend: detected.backend,
            tuning,
          });
          hayateRenderer = new HayateRenderer({
            raw: webHost.raw,
            requestFrame: webHost.requestFrame,
            cancelFrame: webHost.cancelFrame,
          });
          hayateRenderer.start();
          return hayateRenderer;
        },
        stop: () => hayateRenderer?.stop(),
      };

runTsubameApp(host, (renderer) =>
  renderTsubame(() => <TodoApp detected={detected} />, renderer),
);
