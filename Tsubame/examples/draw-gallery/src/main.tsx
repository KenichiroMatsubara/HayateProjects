import { DomRenderer } from '@torimi/tsubame-renderer-dom';
import { HayateRenderer } from '@torimi/tsubame-renderer-hayate';
import { renderTsubame } from '@torimi/tsubame-solid';
import {
  runTsubameApp,
  shouldUseDomRenderer,
  type Host,
} from '@torimi/tsubame-app';
import { DrawGalleryApp } from './App';

const useDomRenderer = shouldUseDomRenderer(window.location.search, {
  hasEditContext: 'EditContext' in window,
});
const dom = document.getElementById('dom-host') as HTMLDivElement;
const canvas = document.getElementById('canvas-stage') as HTMLCanvasElement;

// target（DOM / Hayate）の選択は Host に局在する。合成ルート `runTsubameApp` は
// IRenderer しか知らない（ADR-0012）。draw ギャラリーは todo example と同じ web
// Host adapter の縮小版で、layer-present / tuning などのチューニング口は持たない。
let hayateRenderer: HayateRenderer | undefined;
const host: Host =
  useDomRenderer
    ? {
        // DOM 経路: draw は各 view に敷いた `<canvas>` へ canvas 2D で replay される
        // （Tsubame ADR-0014）。wire も WASM も通らない。
        createRenderer() {
          dom.hidden = false;
          return new DomRenderer({ container: dom });
        },
      }
    : {
        // Hayate 経路: host が backend（vello / tiny-skia）を選び WASM を
        // ロードして surface 上に raw を確立、frame-clock も供給する。draw は wire の
        // `draws` チャネルで運ばれ GPU/CPU ラスタライザが描く。tiny-skia は WebGPU の
        // 無いヘッドレスでも Canvas モードに入れる（e2e の Hayate 経路が使う）。
        async createRenderer() {
          const { createHayateWebHost } = await import('@torimi/hayate-host');
          canvas.hidden = false;
          const webHost = await createHayateWebHost(canvas);
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

runTsubameApp(host, (renderer) => renderTsubame(() => <DrawGalleryApp />, renderer));
