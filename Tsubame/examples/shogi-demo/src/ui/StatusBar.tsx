import { Color } from 'tsshogi';
import type { GameResult } from '../rules/game.js';
import * as S from './styles.js';

/** 手番・手数・王手・決着・思考中を 1 行で出す。 */
export interface StatusBarProps {
  readonly sideToMove: Color;
  readonly ply: number;
  readonly inCheck: boolean;
  readonly outcome: GameResult | null;
  /** AI が思考中なら表示する深さ。思考していなければ `null`。 */
  readonly thinkingDepth: number | null;
}

export function StatusBar({ sideToMove, ply, inCheck, outcome, thinkingDepth }: StatusBarProps) {
  return (
    <view style={S.statusBar}>
      <text style={S.statusMain}>{mainText(sideToMove, inCheck, outcome)}</text>
      <text style={S.statusSub}>
        {thinkingDepth !== null ? `思考中 深さ${thinkingDepth}` : `${ply}手`}
      </text>
    </view>
  );
}

function mainText(sideToMove: Color, inCheck: boolean, outcome: GameResult | null): string {
  if (outcome !== null) return outcomeText(outcome);
  const turn = sideToMove === Color.BLACK ? '☗先手番' : '☖後手番';
  return inCheck ? `${turn}　王手` : turn;
}

function outcomeText(outcome: GameResult): string {
  const winner = (color: Color) => (color === Color.BLACK ? '先手' : '後手');
  switch (outcome.kind) {
    case 'checkmate':
      return `詰み — ${winner(outcome.winner)}の勝ち`;
    case 'resign':
      return `投了 — ${winner(outcome.winner)}の勝ち`;
    case 'repetition-draw':
      return '千日手 — 引き分け';
    case 'perpetual-check':
      return `連続王手の千日手 — ${winner(outcome.winner)}の勝ち`;
  }
}
