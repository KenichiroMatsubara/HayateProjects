import {
  Paint,
  PaintingStyle,
  StrokeJoin,
  TextStyle,
} from '@torimi/tsubame-protocol-generated/recorder';
import type { DrawCanvas } from '@torimi/tsubame-renderer-protocol';
import { PieceType } from 'tsshogi';
import * as C from './palette.js';
import { pieceShape } from './piece-shape.js';
import type { Rect } from './board-metrics.js';

/**
 * 駒を描く唯一の場所。
 *
 * 駒は「五角形」と「漢字」が別々に置かれる 2 つの絵ではなく、**1 つの描画単位**で
 * ある。先後の別は向きだけで表し、それを引数 1 つ（`flipped`）で受ける。図形と文字は
 * 同じ変換の下で描かれるので、片方だけが回るという破れは構造的に起こり得ない。
 *
 * 盤（{@link ../paint/board-painter.js}）と駒台（{@link ./hand-painter.js}）の
 * どちらもこの関数を呼ぶ。盤を 2 度走査したり、駒台にだけ五角形が無かったりしない。
 */

/**
 * マス幅に対する駒文字の大きさ。五角形は頭が細いので、字は駒の胴に収まる幅にする。
 */
const GLYPH_RATIO = 0.46;
/**
 * 字のレイアウトボックス上端を、マス中心からどれだけ上に置くか（字の大きさ比）。
 *
 * ボックスをちょうど中央に置く（0.5）と字は少し上に浮く。CJK の墨がボックスの
 * やや下寄りに来ること、加えて**五角形は頭が尖っていて重心が下にある**ことの
 * 両方から、実測に合わせて浅めに戻して駒の胴へ据える。
 */
const GLYPH_TOP_RATIO = 0.32;

/**
 * 駒の表示文字。成駒は 1 文字の略字（と・杏・圭・全・馬・龍）を使う。
 * いずれもバンドル済みの Noto Sans JP に含まれる常用の字で、マス幅に 1 文字で収まる
 * （日本語グリフの実行時取得は要らない）。
 */
const LABELS: Readonly<Record<PieceType, string>> = {
  [PieceType.PAWN]: '歩',
  [PieceType.LANCE]: '香',
  [PieceType.KNIGHT]: '桂',
  [PieceType.SILVER]: '銀',
  [PieceType.GOLD]: '金',
  [PieceType.BISHOP]: '角',
  [PieceType.ROOK]: '飛',
  [PieceType.KING]: '玉',
  [PieceType.PROM_PAWN]: 'と',
  [PieceType.PROM_LANCE]: '杏',
  [PieceType.PROM_KNIGHT]: '圭',
  [PieceType.PROM_SILVER]: '全',
  [PieceType.HORSE]: '馬',
  [PieceType.DRAGON]: '龍',
};

export function pieceLabel(type: PieceType): string {
  return LABELS[type];
}

/**
 * 描画に要る駒の情報。**先後は含まない** — 先手後手の別は `flipped`（向き）だけで
 * 表すからで、地色を変えるといった「向き以外での区別」をここへ足せないようにしてある。
 * 盤の `Piece` も駒台の駒種もこれを満たす。
 */
export interface PieceFace {
  readonly type: PieceType;
}

/**
 * `rect` の中央に駒を 1 枚描く。`flipped` が真なら 180° 回して相手向きにする
 * （五角形も漢字も一緒に回る）。
 *
 * 成駒の朱字だけは向きと独立した意味を持つので、色でも区別する。
 */
export function paintPiece(
  canvas: DrawCanvas,
  rect: Rect,
  piece: PieceFace,
  flipped: boolean,
): void {
  const cx = rect.x + rect.width / 2;
  const cy = rect.y + rect.height / 2;

  canvas.save();
  // 中心を原点にして回し、戻す。以降の描画はすべてこの変換の下に置かれるので、
  // 図形と文字が別々の向きになりようがない。
  canvas.translate(cx, cy);
  if (flipped) canvas.rotate(Math.PI);
  canvas.translate(-cx, -cy);

  const shape = pieceShape(rect);

  const fill = new Paint();
  fill.style = PaintingStyle.fill;
  fill.color = C.PIECE_FACE;
  canvas.drawPath(shape, fill);

  const edge = new Paint();
  edge.style = PaintingStyle.stroke;
  edge.color = C.PIECE_EDGE;
  edge.strokeWidth = Math.max(0.75, Math.min(rect.width, rect.height) * 0.022);
  edge.strokeJoin = StrokeJoin.round;
  canvas.drawPath(shape, edge);

  const size = Math.min(rect.width, rect.height) * GLYPH_RATIO;
  const style = new TextStyle();
  style.fontSize = size;
  style.fontWeight = 700;

  const ink = new Paint();
  ink.style = PaintingStyle.fill;
  ink.color = isPromoted(piece.type) ? C.PIECE_INK_PROMOTED : C.PIECE_INK;

  // `drawText` の原点はレイアウトボックスの左上。1 文字を駒の中央に据える。
  canvas.drawText(
    pieceLabel(piece.type),
    cx - size / 2,
    cy - size * GLYPH_TOP_RATIO,
    ink,
    style,
  );

  canvas.restore();
}

/** 成駒か（表示上、朱字にする駒）。 */
function isPromoted(type: PieceType): boolean {
  return (
    type === PieceType.PROM_PAWN ||
    type === PieceType.PROM_LANCE ||
    type === PieceType.PROM_KNIGHT ||
    type === PieceType.PROM_SILVER ||
    type === PieceType.HORSE ||
    type === PieceType.DRAGON
  );
}
