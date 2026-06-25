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
 *   EditContext 同期をスキップし、IME はネイティブ GameTextInput が所有した
 *   ままになる。viewport 追従（resize）は native ループが `tree.set_viewport` を
 *   直接駆動するため、ハンドルにも `CanvasRenderer` にも resize 経路は無い
 *   （ADR-0080 を native へ延長, issue #475）。
 * - フレームループは自走させない。`requestFrame` を「最新コールバックを保持する
 *   だけ」のものに差し替え、ネイティブの vsync ループが {@link
 *   AndroidCanvasRendererHandle.pumpFrame} で1フレームずつ駆動する。
 */
export interface AndroidCanvasRendererHandle {
  readonly renderer: CanvasRenderer;
  /** ネイティブ vsync ループが毎フレーム単調増加ミリ秒で1回呼ぶ。保持中の
   * フレームコールバックを実行し、`CanvasRenderer` が次フレームを再登録する。 */
  pumpFrame(timestampMs: number): void;
  /** フレーム駆動を止める。 */
  stop(): void;
}

export type AndroidCanvasRendererOptions = Omit<
  CanvasRendererOptions,
  'canvas' | 'requestFrame' | 'cancelFrame'
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

  // `canvas` は意図的に渡さない（→ 内部で null → EditContext 同期を回避）。
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
    stop(): void {
      pendingFrame = null;
      renderer.stop();
    },
  };
}
