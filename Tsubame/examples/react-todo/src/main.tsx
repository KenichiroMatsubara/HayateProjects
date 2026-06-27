import { renderTsubame } from '@tsubame/react';
import { DomRenderer } from '@tsubame/renderer-dom';
import { HayateRenderer } from '@tsubame/renderer-hayate';
import {
  runTsubameApp,
  detectModeFromSearch,
  type DetectModeResult,
  type Host,
} from '@tsubame/app';
import { App } from './App';

// react も solid と同じ合成ルートに乗る。target（DOM / Hayate）の選択は Host に局在し、
// FW 固有なのは mount の 1 行（`renderTsubame(<App/>, renderer)`）だけ（ADR-0012）。
// これで `vite dev` でも EditContext+WebGPU があれば Hayate に描画する（?renderer=vello で明示）。
// 以前の「react は DOM でしか描かれない」は、Canvas エントリが無く DomRenderer 固定だった
// から（adapter の欠陥ではない）。
function detectModeFromWindow(): DetectModeResult {
  return detectModeFromSearch(window.location.search, {
    hasEditContext: 'EditContext' in window,
    hasWebGPU: 'gpu' in navigator,
  });
}

const detected = detectModeFromWindow();
const dom = document.getElementById('dom-host') as HTMLDivElement;
const canvas = document.getElementById('canvas-stage') as HTMLCanvasElement;

// NOTE: この web Host adapter は solid 例題の main.tsx と同型。2 つ目が揃ったので、
// 次の機会に中立 App 階層パッケージへ抽出する（ADR-0012「1 adapter は仮の seam、2 で本物」）。
let hayateRenderer: HayateRenderer | undefined;
const host: Host =
  detected.mode === 'DOM'
    ? {
        createRenderer() {
          dom.hidden = false;
          return new DomRenderer({ container: dom });
        },
      }
    : {
        async createRenderer() {
          const { createHayateWebHost } = await import('@hayate/host');
          canvas.hidden = false;
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

runTsubameApp(host, (renderer) => renderTsubame(<App />, renderer));
