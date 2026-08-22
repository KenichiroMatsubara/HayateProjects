import { Color, PieceType, Square, type ImmutablePosition } from 'tsshogi';

/**
 * 静的評価。駒得を主、成りやすさ（敵陣への近さ）と玉の安全を従とする素朴な関数。
 * 強さより**速さと素直さ**を採る — 探索が浅い（tsshogi の合法手生成が 1 局面あたり
 * 0.6ms 程度かかる）ので、評価に時間を掛けても割に合わない。
 */

/** 駒の価値（歩＝100）。成駒は元の駒より少し高く見る。 */
const VALUE: Readonly<Record<PieceType, number>> = {
  [PieceType.PAWN]: 100,
  [PieceType.LANCE]: 350,
  [PieceType.KNIGHT]: 400,
  [PieceType.SILVER]: 550,
  [PieceType.GOLD]: 600,
  [PieceType.BISHOP]: 850,
  [PieceType.ROOK]: 1000,
  [PieceType.KING]: 0, // 玉は取られないので価値を入れない（詰みは探索側が扱う）。
  [PieceType.PROM_PAWN]: 600,
  [PieceType.PROM_LANCE]: 600,
  [PieceType.PROM_KNIGHT]: 600,
  [PieceType.PROM_SILVER]: 600,
  [PieceType.HORSE]: 1150,
  [PieceType.DRAGON]: 1300,
};

/** 持ち駒は打てるぶん盤上より少し価値が高い。 */
const HAND_BONUS = 1.1;
/** 敵陣に近い駒を評価する係数（1 段あたり）。 */
const ADVANCE_BONUS = 8;
/** 玉の周りの自駒 1 枚あたりの加点。 */
const GUARD_BONUS = 10;
/** 玉が自陣の底から 1 段離れるごとの減点。囲いを崩して出歩くのを抑える。 */
const KING_EXPOSURE = 45;

/** 駒の価値（他モジュールの指し手順序付けからも使う）。 */
export function pieceValue(type: PieceType): number {
  return VALUE[type];
}

/**
 * `color` から見た評価値。正なら `color` が有利。
 */
export function evaluate(position: ImmutablePosition, color: Color): number {
  let score = 0;

  for (const square of position.board.listNonEmptySquares()) {
    const piece = position.board.at(square);
    if (piece === null) continue;
    const sign = piece.color === color ? 1 : -1;
    let value = VALUE[piece.type];

    if (piece.type !== PieceType.KING) {
      // 先手は段が小さいほど、後手は大きいほど敵陣に近い。
      const advance = piece.color === Color.BLACK ? 9 - square.rank : square.rank;
      value += advance * ADVANCE_BONUS;
    }
    score += sign * value;
  }

  for (const side of [Color.BLACK, Color.WHITE]) {
    const sign = side === color ? 1 : -1;
    const hand = position.hand(side);
    for (const { type, count } of hand.counts) {
      score += sign * count * VALUE[type] * HAND_BONUS;
    }
  }

  score += kingSafety(position, color) - kingSafety(position, opposite(color));
  return Math.round(score);
}

/**
 * 玉の安全度。周囲の自駒の枚数を加点し、**自陣の底から離れた分を減点**する。
 *
 * 減点が要るのは、加点だけだと「歩の群れに玉を潜り込ませる」のが得に見えて、
 * 序盤から玉が出歩いてしまうため（囲いを組むのではなく駒に埋もれに行く）。
 */
function kingSafety(position: ImmutablePosition, color: Color): number {
  const king = findKing(position, color);
  if (king === null) return 0;

  let guards = 0;
  for (let dx = -1; dx <= 1; dx += 1) {
    for (let dy = -1; dy <= 1; dy += 1) {
      if (dx === 0 && dy === 0) continue;
      const neighbor = king.neighbor(dx, dy);
      if (!neighbor.valid) continue;
      if (position.board.at(neighbor)?.color === color) guards += 1;
    }
  }

  // 先手の底は 9 段目、後手は 1 段目。
  const homeRank = color === Color.BLACK ? 9 : 1;
  const advanced = Math.abs(king.rank - homeRank);
  return guards * GUARD_BONUS - advanced * KING_EXPOSURE;
}

function findKing(position: ImmutablePosition, color: Color): Square | null {
  for (const square of position.board.listNonEmptySquares()) {
    const piece = position.board.at(square);
    if (piece?.type === PieceType.KING && piece.color === color) return square;
  }
  return null;
}

function opposite(color: Color): Color {
  return color === Color.BLACK ? Color.WHITE : Color.BLACK;
}
