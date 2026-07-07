import type { DrawCanvas } from './generated/draw-canvas.js';

/** painter が受け取る、レイアウト確定済みボーダーボックスの論理 px サイズ。 */
export interface DrawSize {
  readonly width: number;
  readonly height: number;
}

/** `draw` property の painter オブジェクト形（Flutter CustomPainter 同型・ADR-0141）。 */
export interface DrawPainter {
  /** ボーダーボックス左上原点・論理 px・DPR 不可視で記録面へ描く。 */
  paint(canvas: DrawCanvas, size: DrawSize): void;
  /**
   * property 再設定時に前回 painter と比べて再描画が要るか。
   * 省略時は毎回再描画（保守的既定）。
   */
  shouldRepaint?(oldPainter: DrawPainter): boolean;
}

/** painter の関数糖衣。再描画判定は identity 比較。 */
export type DrawPaintFunction = (canvas: DrawCanvas, size: DrawSize) => void;

/** `draw` property の値。生 display list は公開しない（#730）。 */
export type DrawProperty = DrawPainter | DrawPaintFunction;

/**
 * property 再設定時の再記録・再送信要否（reactive 無効化・#730）。同一値は常に
 * スキップ、関数糖衣は identity 比較、painter オブジェクトは `shouldRepaint(old)`
 * に委ねる。判定意味論はこの 1 関数だけが持ち、各レンダラーは再宣言しない
 *（Semantics Parity・Tsubame ADR-0008 の流儀）。
 */
export function drawNeedsRepaint(
  next: DrawProperty,
  prev: DrawProperty | undefined,
): boolean {
  if (prev === undefined) return true;
  if (next === prev) return false;
  if (typeof next === 'function' || typeof prev === 'function') return true;
  return next.shouldRepaint?.(prev) ?? true;
}

/** painter オブジェクト / 関数糖衣の差を吸収して 1 回描かせる。 */
export function invokePainter(
  value: DrawProperty,
  canvas: DrawCanvas,
  size: DrawSize,
): void {
  if (typeof value === 'function') {
    value(canvas, size);
  } else {
    value.paint(canvas, size);
  }
}
