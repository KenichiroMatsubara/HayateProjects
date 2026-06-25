import type { RawHayate } from './raw-hayate.js';

export type { RawHayate } from './raw-hayate.js';

/**
 * Android（ネイティブ Hayate + 埋め込み JS エンジン, ADR-0112）向けの host。web の
 * {@link import('./index.js').WebHost} と対称な形で `raw` + frame-clock を供給するが、
 * clock はネイティブ vsync ループが 1 フレームずつ駆動する pump になっている。
 *
 * web bootstrap と違い WASM をロードしない：Hayate はネイティブ cdylib として既に存在し、
 * ホスト（JSI HostObject 等）が {@link RawHayate} を満たすオブジェクトを JS へ注入する。
 * surface・resize・IME は native（GameTextInput / native ループの `tree.set_viewport`）が
 * 所有し、JS 経路には無い（ADR-0080 を native へ延長, #475 / #474）。
 */
export interface NativeHost {
  readonly raw: RawHayate;
  /** host-blind コアが次フレームを要求する。最新コールバックを保持するだけで自走しない。 */
  readonly requestFrame: (cb: FrameRequestCallback) => number;
  readonly cancelFrame: (handle: number) => void;
  /** ネイティブ vsync ループが毎フレーム単調増加ミリ秒で 1 回呼ぶ。保持中のフレーム
   * コールバックを実行し、`CanvasRenderer` が次フレームを再登録する。 */
  pumpFrame(timestampMs: number): void;
  /** フレーム駆動を止める。 */
  stop(): void;
}

/**
 * 注入された {@link RawHayate} を pump 型 frame-clock に結線して {@link NativeHost} を返す。
 * `requestFrame` は最新コールバックを保持するだけ。`CanvasRenderer.frame` は末尾で再登録
 * するので、1 回の `pumpFrame` が 1 フレームを走らせ、次フレーム分を再武装する。
 */
export function createHayateNativeHost(raw: RawHayate): NativeHost {
  let pendingFrame: FrameRequestCallback | null = null;
  let handleSeq = 1;

  return {
    raw,
    requestFrame(cb: FrameRequestCallback): number {
      pendingFrame = cb;
      return handleSeq++;
    },
    cancelFrame(_handle: number): void {
      pendingFrame = null;
    },
    pumpFrame(timestampMs: number): void {
      const cb = pendingFrame;
      pendingFrame = null;
      cb?.(timestampMs);
    },
    stop(): void {
      pendingFrame = null;
    },
  };
}
