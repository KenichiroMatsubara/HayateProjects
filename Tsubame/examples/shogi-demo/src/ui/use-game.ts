import { PieceType, Square, type Move } from 'tsshogi';
import type { MoveSource, ShogiGame } from '../rules/game.js';

/**
 * 盤・駒台のタップを指し手に変える状態機械。**React を知らない純関数**として書き、
 * 選択→移動→成り選択の流れを単体テストで固められるようにする。
 */

/** 何を掴んでいるか。 */
export type Selection =
  | { readonly kind: 'none' }
  | { readonly kind: 'board'; readonly from: Square }
  | { readonly kind: 'hand'; readonly pieceType: PieceType };

export const NO_SELECTION: Selection = { kind: 'none' };

/** 成りを問うている最中の保留手。 */
export interface PendingPromotion {
  readonly from: MoveSource;
  readonly to: Square;
}

export interface InteractionState {
  readonly selection: Selection;
  readonly pending: PendingPromotion | null;
}

export const INITIAL_INTERACTION: InteractionState = {
  selection: NO_SELECTION,
  pending: null,
};

/** リデューサが返す「次の状態」と「指すべき手」。手は呼び出し側が `game.play` に渡す。 */
export interface InteractionResult {
  readonly state: InteractionState;
  readonly move: Move | null;
}

export type InteractionAction =
  | { readonly kind: 'tap-square'; readonly square: Square }
  | { readonly kind: 'tap-hand'; readonly pieceType: PieceType }
  | { readonly kind: 'choose-promotion'; readonly promote: boolean }
  | { readonly kind: 'cancel' };

/**
 * タップを解釈する。`game` は読むだけで、着手そのものは呼び出し側が行う
 * （リデューサを純粋に保ち、AI の手番や決着後の抑止を 1 箇所で見られるようにする）。
 */
export function reduceInteraction(
  game: ShogiGame,
  state: InteractionState,
  action: InteractionAction,
): InteractionResult {
  if (action.kind === 'cancel') return { state: INITIAL_INTERACTION, move: null };

  if (action.kind === 'choose-promotion') {
    if (state.pending === null) return { state, move: null };
    const move = game.createMove(state.pending.from, state.pending.to, action.promote);
    return { state: INITIAL_INTERACTION, move };
  }

  if (action.kind === 'tap-hand') {
    // 同じ駒種を二度叩いたら選択解除。持っていない駒は掴めない。
    if (state.selection.kind === 'hand' && state.selection.pieceType === action.pieceType) {
      return { state: INITIAL_INTERACTION, move: null };
    }
    if (game.position.hand(game.sideToMove).count(action.pieceType) === 0) {
      return { state: INITIAL_INTERACTION, move: null };
    }
    return {
      state: { selection: { kind: 'hand', pieceType: action.pieceType }, pending: null },
      move: null,
    };
  }

  const { square } = action;
  const source = selectionSource(state.selection);

  if (source !== null) {
    const choice = game.promotionChoice(source, square);
    if (choice === 'optional') {
      // 成・不成の両方が合法なので、指す前に問う。
      return { state: { selection: state.selection, pending: { from: source, to: square } }, move: null };
    }
    if (choice === 'forced' || choice === 'none') {
      return {
        state: INITIAL_INTERACTION,
        move: game.createMove(source, square, choice === 'forced'),
      };
    }
    // 非合法な行き先。自分の別の駒なら掴み直し、それ以外は解除。
    if (source instanceof Square && source.equals(square)) {
      return { state: INITIAL_INTERACTION, move: null };
    }
  }

  const piece = game.position.board.at(square);
  if (piece !== null && piece.color === game.sideToMove) {
    return { state: { selection: { kind: 'board', from: square }, pending: null }, move: null };
  }
  return { state: INITIAL_INTERACTION, move: null };
}

/** 選択中の移動元（盤のマス or 打つ駒種）。未選択なら `null`。 */
export function selectionSource(selection: Selection): MoveSource | null {
  if (selection.kind === 'board') return selection.from;
  if (selection.kind === 'hand') return selection.pieceType;
  return null;
}
