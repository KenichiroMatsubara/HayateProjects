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

// HostSession が renderer と worker host の共有 lifetime をまとめ、Composition Root には
// renderer だけを公開する（ADR-0015）。
const host: Host =
  useDomRenderer
    ? {
        start() {
          dom.hidden = false;
          return {
            renderer: new DomRenderer({ container: dom }),
            dispose() {},
          };
        },
      }
    : {
        async start() {
          const { createHayateWebHost } = await import('@torimi/hayate-host');
          canvas.hidden = false;
          const tuning = await fetch(new URL('tuning.jsonc', document.baseURI).href)
            .then((r) => (r.ok ? r.text() : undefined))
            .catch(() => undefined);
          const webHost = await createHayateWebHost(canvas, {
            tuning,
          });
          const renderer = new HayateRenderer({
            raw: webHost.raw,
            requestFrame: webHost.requestFrame,
            cancelFrame: webHost.cancelFrame,
          });
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

runTsubameApp(host, (renderer) => renderTsubame(<App />, renderer));
