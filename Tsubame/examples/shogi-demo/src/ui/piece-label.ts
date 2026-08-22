import { PieceType } from 'tsshogi';

/**
 * 駒の表示文字。成駒は 1 文字の略字（と・杏・圭・全・馬・龍）を使う。
 * いずれも Noto Sans JP に含まれる常用の字で、マス幅に 1 文字で収まる。
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
