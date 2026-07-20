import { DomRenderer } from '@torimi/tsubame-renderer-dom';
import { HayateRenderer } from '@torimi/tsubame-renderer-hayate';
import { renderTsubame } from '@torimi/tsubame-solid';
import {
  runTsubameApp,
  shouldUseDomRenderer,
  type Host,
} from '@torimi/tsubame-app';
import { TodoApp } from './App';

const useDomRenderer = shouldUseDomRenderer(window.location.search, {
  hasEditContext: 'EditContext' in window,
});
const dom = document.getElementById('dom-host') as HTMLDivElement;
const canvas = document.getElementById('canvas-stage') as HTMLCanvasElement;

// target（DOM / Hayate）の選択は Host に局在する。合成ルート `runTsubameApp` は IRenderer
// しか知らず、DomRenderer / HayateRenderer も WebGPU プローブも見ない（ADR-0012）。
// NOTE: 下の web Host adapter は react-demo の main.tsx と同型になる見込み。2 つ目が出た
// 時点で中立 App 階層パッケージへ抽出する（ADR-0012「1 adapter は仮の seam、2 で本物」）。
let hayateRenderer: HayateRenderer | undefined;
const host: Host =
  useDomRenderer
    ? {
        // DOM 経路：Hayate を迂回し、native IME と CSS リフローに委ねる。viewport 追従は
        // ブラウザの CSS / `@media` が担い、Tsubame は resize を配線しない（ADR-0080）。
        createRenderer() {
          dom.hidden = false;
          return new DomRenderer({ container: dom });
        },
      }
    : {
        // Hayate 経路：host bootstrap は Hayate 側（`@torimi/hayate-host`）が持つ。host が WebGPU を
        // プローブし backend を選び WASM をロードして surface 上に raw を確立し、frame-clock も
        // 供給する。App は host から raw(+clock) を得て host-blind HayateRenderer に結線するだけ。
        async createRenderer() {
          const { createHayateWebHost } = await import('@torimi/hayate-host');
          canvas.hidden = false;
          // Dev-only: 配信ルートの手書き `tuning.jsonc` を拾い、F5 だけで感触定数を較正できる
          // （WASM 再ビルド不要）。404 / parse 失敗はコンパイル既定のまま。
          const tuning = await fetch(new URL('tuning.jsonc', document.baseURI).href)
            .then((r) => (r.ok ? r.text() : undefined))
            .catch(() => undefined);
          const webHost = await createHayateWebHost(canvas, {
            tuning,
          });
          hayateRenderer = new HayateRenderer({
            raw: webHost.raw,
            requestFrame: webHost.requestFrame,
            cancelFrame: webHost.cancelFrame,
          });
          // e2e が Hayate のレイアウト正本（element_get_bounds / element_subtree_ids 等）を
          // 照会するための debug seam。canvas は DOM から黒箱なので、レイアウト回帰の
          // 数値アサートはこれを使う（e2e/canvas-gallery-section-overlap.spec.ts）。
          (window as unknown as { __hayateRaw?: unknown }).__hayateRaw = webHost.raw;
          hayateRenderer.start();
          return hayateRenderer;
        },
        stop: () => hayateRenderer?.stop(),
      };

runTsubameApp(host, (renderer) => renderTsubame(() => <TodoApp />, renderer));
