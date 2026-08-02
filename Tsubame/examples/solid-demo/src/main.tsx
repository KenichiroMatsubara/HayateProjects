import { DomRenderer } from '@torimi/tsubame-renderer-dom';
import { HayateRenderer } from '@torimi/tsubame-renderer-hayate';
import { renderTsubame } from '@torimi/tsubame-solid';
import {
  runTsubameApp,
  shouldUseDomRenderer,
  type Host,
} from '@torimi/tsubame-app';
import { TodoApp } from './App';
import { WorkerScrollWorkload } from './WorkerScrollWorkload';

const useDomRenderer = shouldUseDomRenderer(window.location.search, {
  hasEditContext: 'EditContext' in window,
});
const useWorkerScrollWorkload =
  new URLSearchParams(window.location.search).get('workload') === 'worker-scroll';
const dom = document.getElementById('dom-host') as HTMLDivElement;
const canvas = document.getElementById('canvas-stage') as HTMLCanvasElement;

// target（DOM / Hayate）の選択は Host に局在する。合成ルート `runTsubameApp` は IRenderer
// しか知らず、DomRenderer / HayateRenderer も WebGPU プローブも見ない（ADR-0012）。
// HostSession が renderer と worker host の共有 lifetime をまとめ、Composition Root には
// renderer だけを公開する（ADR-0015）。
const host: Host =
  useDomRenderer
    ? {
        // DOM 経路：Hayate を迂回し、native IME と CSS リフローに委ねる。viewport 追従は
        // ブラウザの CSS / `@media` が担い、Tsubame は resize を配線しない（ADR-0080）。
        start() {
          dom.hidden = false;
          return {
            renderer: new DomRenderer({ container: dom }),
            dispose() {},
          };
        },
      }
    : {
        // Hayate 経路：host bootstrap は Hayate 側（`@torimi/hayate-host`）が持つ。標準 host は
        // OffscreenCanvas と単一 Worker を確立し、Rust RenderHost が Scene Renderer を選択する。
        // App は host から raw(+clock) を得て host-blind HayateRenderer に結線するだけ。
        async start() {
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
          (
            window as unknown as {
              __hayateHost?: typeof webHost;
            }
          ).__hayateHost = webHost;
          const renderer = new HayateRenderer({
            raw: webHost.raw,
            requestFrame: webHost.requestFrame,
            cancelFrame: webHost.cancelFrame,
          });
          // Canvas は DOM から黒箱なので、局所的なデバッグ時はこの raw seam を使う。
          (window as unknown as { __hayateRaw?: unknown }).__hayateRaw = webHost.raw;
          renderer.start();
          let disposed = false;
          return {
            renderer,
            dispose() {
              if (disposed) return;
              disposed = true;
              try {
                renderer.stop();
              } finally {
                webHost.detach();
              }
            },
          };
        },
      };

runTsubameApp(host, (renderer) =>
  renderTsubame(
    () => (useWorkerScrollWorkload ? <WorkerScrollWorkload /> : <TodoApp />),
    renderer,
  ),
);
