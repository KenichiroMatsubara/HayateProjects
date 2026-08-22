import type { DrawSize } from '@torimi/tsubame-renderer-protocol';

/**
 * 盤の幾何。**純関数だけ**を置き、描画にも当たり判定にも同じ値を使わせる
 * （盤の線と 81 個のセル要素がずれないための単一の出所）。
 */

/** 矩形（論理 px・painter の box 左上原点）。 */
export interface Rect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

/** 盤の外枠の太さが、盤全体の短辺に占める割合。 */
const EDGE_RATIO = 0.018;

export interface BoardMetrics {
  /** 外枠を含む盤全体。 */
  readonly board: Rect;
  /**
   * 枡目が占める領域。**`board` と一致する**。
   *
   * 外枠は内側へ描き込む（枡目を内側へ押し込まない）。要素側は同じ箱を 9 等分した
   * grid でタップ領域を敷くので、枡目を内側へ寄せると painter の絵と当たり判定が
   * その分だけずれる — 枡目と要素グリッドを同じ割り付けにしておくのが、この
   * モジュールが「単一の出所」である理由そのもの。
   */
  readonly grid: Rect;
  /** 1 マスの一辺。 */
  readonly cell: number;
  /** 外枠の太さ。 */
  readonly edge: number;
}

/**
 * painter の box サイズから盤の幾何を決める。box が正方形でなくても短辺に合わせ、
 * 余りは中央寄せで捨てる（要素側は `aspectRatio: 1` を敷くので通常は正方形で来る）。
 */
export function boardMetrics(size: DrawSize): BoardMetrics {
  const side = Math.min(size.width, size.height);
  const edge = Math.max(1, side * EDGE_RATIO);
  const board: Rect = {
    x: (size.width - side) / 2,
    y: (size.height - side) / 2,
    width: side,
    height: side,
  };
  return {
    board,
    grid: board,
    cell: side / 9,
    edge,
  };
}

/**
 * マスのインデックス（tsshogi の `Square.index`: 0 = 9一 … 80 = 1九）を矩形へ写す。
 * `flipped` が真なら盤を 180° 反転して後手視点にする。
 */
export function squareRect(metrics: BoardMetrics, index: number, flipped: boolean): Rect {
  const view = flipped ? 80 - index : index;
  const column = view % 9;
  const row = Math.floor(view / 9);
  return {
    x: metrics.grid.x + column * metrics.cell,
    y: metrics.grid.y + row * metrics.cell,
    width: metrics.cell,
    height: metrics.cell,
  };
}

/**
 * 画面上の並び順（0 = 左上）から盤のマスインデックスへ戻す。`squareRect` の逆写像で、
 * 要素側が 9×9 grid の子を並べる順序と盤座標を突き合わせるのに使う。
 * `flipped` を二度適用すると元に戻る（対合）。
 */
export function indexAtScreenOrder(order: number, flipped: boolean): number {
  return flipped ? 80 - order : order;
}
