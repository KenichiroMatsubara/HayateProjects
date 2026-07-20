import { renderTsubame } from '@torimi/tsubame-react';
import { DomRenderer } from '@torimi/tsubame-renderer-dom';
import { HayateRenderer } from '@torimi/tsubame-renderer-hayate';
import {
  runTsubameApp,
  shouldUseDomRenderer,
  type Host,
} from '@torimi/tsubame-app';
import { App } from './App';

// react も solid と同じ合成ルートに乗る。target（DOM / Hayate）の選択は Host に局在し、
// FW 固有なのは mount の 1 行（`renderTsubame(<App/>, renderer)`）だけ（ADR-0012）。
// `vite dev` でも EditContext があれば Hayate に描画し、Auto の backend 順序は Host に委ねる。
// 以前の「react は DOM でしか描かれない」は、Canvas エントリが無く DomRenderer 固定だった
// から（adapter の欠陥ではない）。
const useDomRenderer = shouldUseDomRenderer(window.location.search, {
  hasEditContext: 'EditContext' in window,
});
const dom = document.getElementById('dom-host') as HTMLDivElement;
const canvas = document.getElementById('canvas-stage') as HTMLCanvasElement;

// NOTE: この web Host adapter は solid 例題の main.tsx と同型。2 つ目が揃ったので、
// 次の機会に中立 App 階層パッケージへ抽出する（ADR-0012「1 adapter は仮の seam、2 で本物」）。
let hayateRenderer: HayateRenderer | undefined;
const host: Host =
  useDomRenderer
    ? {
        createRenderer() {
          dom.hidden = false;
          return new DomRenderer({ container: dom });
        },
      }
    : {
        async createRenderer() {
          const { createHayateWebHost } = await import('@torimi/hayate-host');
          canvas.hidden = false;
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
          hayateRenderer.start();
          return hayateRenderer;
        },
        stop: () => hayateRenderer?.stop(),
      };

runTsubameApp(host, (renderer) => renderTsubame(<App />, renderer));
