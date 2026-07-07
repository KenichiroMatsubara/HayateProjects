import { DomRenderer } from '@tsubame/renderer-dom';
import { HayateRenderer } from '@tsubame/renderer-hayate';
import { renderTsubame } from '@tsubame/solid';
import {
  runTsubameApp,
  detectModeFromSearch,
  type DetectModeResult,
  type Host,
} from '@tsubame/app';
import { DrawGalleryApp } from './App';

function detectModeFromWindow(): DetectModeResult {
  return detectModeFromSearch(window.location.search, {
    hasEditContext: 'EditContext' in window,
    hasWebGPU: 'gpu' in navigator,
  });
}

const detected = detectModeFromWindow();
const dom = document.getElementById('dom-host') as HTMLDivElement;
const canvas = document.getElementById('canvas-stage') as HTMLCanvasElement;

// target（DOM / Hayate）の選択は Host に局在する。合成ルート `runTsubameApp` は
// IRenderer しか知らない（ADR-0012）。draw ギャラリーは todo example と同じ web
// Host adapter の縮小版で、layer-present / tuning などのチューニング口は持たない。
let hayateRenderer: HayateRenderer | undefined;
const host: Host =
  detected.mode === 'DOM'
    ? {
        // DOM 経路: draw は各 view に敷いた `<canvas>` へ canvas 2D で replay される
        // （Tsubame ADR-0014）。wire も WASM も通らない。
        createRenderer() {
          dom.hidden = false;
          return new DomRenderer({ container: dom });
        },
      }
    : {
        // Hayate 経路: host が backend（vello / tiny-skia / vello-cpu）を選び WASM を
        // ロードして surface 上に raw を確立、frame-clock も供給する。draw は wire の
        // `draws` チャネルで運ばれ GPU/CPU ラスタライザが描く。tiny-skia は WebGPU の
        // 無いヘッドレスでも Canvas モードに入れる（e2e の Hayate 経路が使う）。
        async createRenderer() {
          const { createHayateWebHost } = await import('@hayate/host');
          canvas.hidden = false;
          const webHost = await createHayateWebHost(canvas, { backend: detected.backend });
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
  renderTsubame(() => <DrawGalleryApp detected={detected} />, renderer),
);
