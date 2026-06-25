import { CanvasRenderer, type RawHayate } from '@tsubame/renderer-canvas';
import type { IRenderer } from '@tsubame/renderer-protocol';

/**
 * browser / native 双方の host が満たす最小契約：Hayate ランタイムのポート `raw` と、
 * host が確立した frame-clock（`requestFrame`/`cancelFrame`）。web は WebGPU プローブ +
 * WASM ロードの末に、native は注入 RawHayate + vsync pump として、これを供給する。
 */
export interface CanvasHost {
  readonly raw: RawHayate;
  readonly requestFrame: (cb: FrameRequestCallback) => number;
  readonly cancelFrame: (handle: number) => void;
}

/**
 * host から得た `raw`(+clock) を host-blind `CanvasRenderer` に結線し、開始し、App を
 * mount する対称合成ルート。browser（`main.tsx`）と native（`main.android.tsx`）は
 * この 1 形に縮約される — surface・WASM ロード・clock 源・native pump は host が所有し、
 * Tsubame は host を知らない（#477）。返した renderer で native pump は停止できる。
 */
export function mountCanvasApp(
  host: CanvasHost,
  mount: (renderer: IRenderer) => void,
): CanvasRenderer {
  const renderer = new CanvasRenderer({
    raw: host.raw,
    requestFrame: host.requestFrame,
    cancelFrame: host.cancelFrame,
  });
  renderer.start();
  mount(renderer);
  return renderer;
}
