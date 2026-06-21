import { CanvasRenderer } from './canvas-renderer.js';
import type { CanvasRendererOptions } from './canvas-renderer.js';
import type { RawHayate } from './hayate.js';

/**
 * Android（ネイティブ Hayate + 埋め込み JS エンジン）向けの初期化経路。ADR-0112。
 *
 * ブラウザ用の {@link import('./init.js').initCanvasRenderer} は `navigator.gpu`
 * プローブと WASM インスタンス化を行うが、Android では Hayate はネイティブ cdylib
 * として既に存在する。ホスト（JSI HostObject 等）が {@link RawHayate} を満たす
 * オブジェクトとして JS グローバルへ注入され、この関数がそれを `CanvasRenderer`
 * に結線する。
 *
 * ブラウザ依存の排除点（ADR-0112 の設計どおり）:
 * - `canvas` を渡さない → `CanvasRenderer` 内の `canvas !== null` ガードが
 *   EditContext 同期と `ResizeObserver` の自己結線をスキップし、IME は
 *   ネイティブ GameTextInput が所有したままになる。
 * - フレームループは自走させない。`requestFrame` を「最新コールバックを保持する
 *   だけ」のものに差し替え、ネイティブの vsync ループが {@link
 *   AndroidCanvasRendererHandle.pumpFrame} で1フレームずつ駆動する。
 */
export interface AndroidCanvasRendererHandle {
  readonly renderer: CanvasRenderer;
  /** ネイティブ vsync ループが毎フレーム単調増加ミリ秒で1回呼ぶ。保持中の
   * フレームコールバックを実行し、`CanvasRenderer` が次フレームを再登録する。 */
  pumpFrame(timestampMs: number): void;
  /** サーフェス生成/リサイズ時にネイティブから呼ぶ。`raw.on_resize` 経由で
   * ビューポートへ反映する（DPR は `scale`）。 */
  resize(width: number, height: number, scale?: number): void;
  /** フレーム駆動を止める。 */
  stop(): void;
}

export type AndroidCanvasRendererOptions = Omit<
  CanvasRendererOptions,
  'canvas' | 'autoResize' | 'requestFrame' | 'cancelFrame' | 'createResizeObserver'
>;

export function createAndroidCanvasRenderer(
  raw: RawHayate,
  options?: AndroidCanvasRendererOptions,
): AndroidCanvasRendererHandle {
  // ネイティブ駆動フレームポンプ。`requestFrame` は最新コールバックを保持する
  // だけで自走しない。`CanvasRenderer.frame` は末尾で再登録するので、1回の
  // `pumpFrame` が1フレームを走らせ、次フレーム分を再武装する。
  let pendingFrame: FrameRequestCallback | null = null;
  let handleSeq = 1;

  const requestFrame = (cb: FrameRequestCallback): number => {
    pendingFrame = cb;
    return handleSeq++;
  };
  const cancelFrame = (_handle: number): void => {
    pendingFrame = null;
  };

  // `canvas` は意図的に渡さない（→ 内部で null → autoResize=false、
  // EditContext/ResizeObserver の自己結線を回避）。
  const renderer = new CanvasRenderer(raw, {
    ...options,
    requestFrame,
    cancelFrame,
  });

  return {
    renderer,
    pumpFrame(timestampMs: number): void {
      const cb = pendingFrame;
      pendingFrame = null;
      cb?.(timestampMs);
    },
    resize(width: number, height: number, scale = 1): void {
      renderer.resize(width, height, scale);
    },
    stop(): void {
      pendingFrame = null;
      renderer.stop();
    },
  };
}
