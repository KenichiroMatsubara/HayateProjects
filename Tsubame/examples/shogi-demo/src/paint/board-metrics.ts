/**
 * 盤の幾何。**純関数だけ**を置く。
 *
 * 枡の位置と大きさはここには無い — 盤は 9×9 の grid 要素で、枡は要素そのものだから
 * （レイアウトエンジンが持つ）。painter は自分の箱（`DrawSize`）の中だけを描くので、
 * 「絵の座標」と「当たり判定の座標」が二重に計算されることがそもそも起こらない。
 * ここに残るのは、画面の並び順と盤のマス番号を突き合わせる写像だけ。
 */

/** 矩形（論理 px・painter の箱の左上原点）。 */
export interface Rect {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

/** painter の箱いっぱいの矩形。painter は自分の箱しか知らないので、原点は常に 0。 */
export function boxRect(size: { readonly width: number; readonly height: number }): Rect {
  return { x: 0, y: 0, width: size.width, height: size.height };
}

/**
 * 画面上の並び順（0 = 左上）から盤のマスインデックス（tsshogi の `Square.index`:
 * 0 = 9一 … 80 = 1九）へ写す。`flipped` が真なら盤を 180° 回して後手視点にする。
 * 二度適用すると元に戻る（対合）。
 */
export function indexAtScreenOrder(order: number, flipped: boolean): number {
  return flipped ? 80 - order : order;
}

/**
 * その枡の**左上の角**が星（`hoshi`）の交点か。星は枡の中ではなく枡と枡の交点に打つので、
 * 交点を右下に持つ枡が自分の箱の外へはみ出して描く（`overflow: visible` が既定）。
 *
 * 盤面上の位置は 180° 回転で不変なので、判定は盤のマス番号ではなく**画面の並び順**で行う。
 */
export function isHoshiCorner(order: number): boolean {
  const column = order % 9;
  const row = Math.floor(order / 9);
  return (column === 3 || column === 6) && (row === 3 || row === 6);
}
