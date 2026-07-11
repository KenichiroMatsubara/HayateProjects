import type { DrawSize } from '@torimi/tsubame-renderer-protocol';

/**
 * `overflow: visible`（既定）の view で box 外への描画が切れないよう、draw 用
 * `<canvas>` を box より各辺この論理 px ぶん大きく確保する（#731 / Tsubame
 * ADR-0014）。太い stroke・グロー・はみ出す図形の典型を覆う値で、コストは
 * canvas のメモリ（面積 × DPR²）。将来の実測チューニング対象。
 */
export const DRAW_OVERFLOW_VISIBLE_MARGIN_PX = 64;

/** draw surface が従う view の overflow（既定 visible・ADR-0141）。 */
export type DrawSurfaceOverflow = 'visible' | 'hidden';

/**
 * draw 用 `<canvas>` の配置と解像度。css* は view ローカルの論理 px（canvas の
 * style）、device* は物理ピクセル（canvas の width/height 属性）、origin* は
 * painter の原点（box 左上）が canvas 内でどこに来るか（論理 px）。
 */
export interface DrawSurfaceGeometry {
  readonly cssLeft: number;
  readonly cssTop: number;
  readonly cssWidth: number;
  readonly cssHeight: number;
  readonly deviceWidth: number;
  readonly deviceHeight: number;
  readonly originX: number;
  readonly originY: number;
}

/**
 * レイアウト確定サイズ・DPR・overflow から draw surface の敷き方を決める純関数。
 * 物理解像度 = 論理サイズ × devicePixelRatio（DPR は painter から不可視）。
 * `visible` は box の四辺に {@link DRAW_OVERFLOW_VISIBLE_MARGIN_PX} を足して
 * はみ出しを保持し、`hidden` はどうせ box で CSS クリップされるので box ぴったり
 * に確保する。
 */
export function drawSurfaceGeometry(
  size: DrawSize,
  devicePixelRatio: number,
  overflow: DrawSurfaceOverflow,
): DrawSurfaceGeometry {
  const margin = overflow === 'visible' ? DRAW_OVERFLOW_VISIBLE_MARGIN_PX : 0;
  const cssWidth = size.width + 2 * margin;
  const cssHeight = size.height + 2 * margin;
  // `0 - 0` は -0 になり Object.is 同値性で +0 と区別されるため明示的に潰す。
  const offset = margin === 0 ? 0 : -margin;
  return {
    cssLeft: offset,
    cssTop: offset,
    cssWidth,
    cssHeight,
    // 端数 DPR（1.5 等）で 1 物理 px 欠けないよう切り上げる。
    deviceWidth: Math.ceil(cssWidth * devicePixelRatio),
    deviceHeight: Math.ceil(cssHeight * devicePixelRatio),
    originX: margin,
    originY: margin,
  };
}
