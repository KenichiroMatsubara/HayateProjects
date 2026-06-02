import initWasm, { HayateElementRenderer } from 'hayate-adapter-web';
import { CanvasRenderer } from './canvas-renderer.js';
import type { CanvasRendererOptions } from './canvas-renderer.js';
import type { RawHayate } from './hayate.js';

/**
 * 実 Hayate WASM を初期化し `CanvasRenderer` を返す。
 *
 * アプリ側はこの関数を呼ぶだけでよく、WASM ロードや `HayateElementRenderer`
 * のライフサイクルを意識する必要がない。`HayateElementRenderer` は WIT
 * element-layer の wasm-bindgen 実装で、構造的に {@link RawHayate} を充足する。
 */
export async function initCanvasRenderer(
  canvas: HTMLCanvasElement,
  options?: CanvasRendererOptions,
): Promise<CanvasRenderer> {
  await initWasm();
  const raw = (await HayateElementRenderer.init(canvas)) as unknown as RawHayate;
  attachPointerInput(canvas, raw);
  return new CanvasRenderer(raw, options);
}

/**
 * ポインタ入力を実 Canvas renderer に供給する。
 *
 * DOM Mode と異なり Canvas renderer はブラウザのヒットテストを持たないため、
 * ホストが canvas 座標系の生 (x, y) を `on_pointer_*` に渡す責務を負う。
 * renderer 内部でヒットテストして click / hover-enter / hover-leave を生成し、
 * `poll_events()` 経由で返す。これが無いと一切のインタラクションが発火しない。
 */
function attachPointerInput(canvas: HTMLCanvasElement, raw: RawHayate): void {
  const toCanvas = (e: MouseEvent): readonly [number, number] => {
    const rect = canvas.getBoundingClientRect();
    const sx = rect.width === 0 ? 1 : canvas.width / rect.width;
    const sy = rect.height === 0 ? 1 : canvas.height / rect.height;
    return [(e.clientX - rect.left) * sx, (e.clientY - rect.top) * sy];
  };
  canvas.addEventListener('mousemove', (e) => {
    const [x, y] = toCanvas(e);
    raw.on_pointer_move(x, y);
  });
  canvas.addEventListener('mousedown', (e) => {
    const [x, y] = toCanvas(e);
    raw.on_pointer_down(x, y);
  });
  canvas.addEventListener('mouseup', (e) => {
    const [x, y] = toCanvas(e);
    raw.on_pointer_up(x, y);
  });
  canvas.addEventListener(
    'wheel',
    (e) => {
      const [x, y] = toCanvas(e);
      raw.on_wheel(x, y, e.deltaX, e.deltaY);
    },
    { passive: true },
  );
}
