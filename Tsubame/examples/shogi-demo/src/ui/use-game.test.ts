import { describe, expect, it } from 'vitest';
import { PieceType, Position, Square } from 'tsshogi';
import { ShogiGame } from '../rules/game.js';
import {
  INITIAL_INTERACTION,
  reduceInteraction,
  selectionSource,
  type InteractionState,
} from './use-game.js';

/** タップ列を順に流し、指し手が出たら着手して、最後の状態を返す。 */
function drive(
  game: ShogiGame,
  actions: readonly Parameters<typeof reduceInteraction>[2][],
): InteractionState {
  let state = INITIAL_INTERACTION;
  for (const action of actions) {
    const result = reduceInteraction(game, state, action);
    state = result.state;
    if (result.move !== null) game.play(result.move);
  }
  return state;
}

describe('reduceInteraction — 盤上の選択', () => {
  it('自分の駒を叩くと選択され、移動先を叩くと着手される', () => {
    const game = new ShogiGame();
    const state = drive(game, [
      { kind: 'tap-square', square: new Square(7, 7) },
      { kind: 'tap-square', square: new Square(7, 6) },
    ]);
    expect(state).toEqual(INITIAL_INTERACTION);
    expect(game.lastMove?.usi).toBe('7g7f');
  });

  it('同じ駒をもう一度叩くと選択が解除される', () => {
    const game = new ShogiGame();
    const state = drive(game, [
      { kind: 'tap-square', square: new Square(7, 7) },
      { kind: 'tap-square', square: new Square(7, 7) },
    ]);
    expect(state.selection.kind).toBe('none');
    expect(game.ply).toBe(0);
  });

  it('別の自駒を叩くと掴み直しになる', () => {
    const game = new ShogiGame();
    const state = drive(game, [
      { kind: 'tap-square', square: new Square(7, 7) },
      { kind: 'tap-square', square: new Square(2, 7) },
    ]);
    expect(selectionSource(state.selection)).toBeInstanceOf(Square);
    expect((selectionSource(state.selection) as Square).file).toBe(2);
    expect(game.ply).toBe(0);
  });

  it('相手の駒は掴めない', () => {
    const game = new ShogiGame();
    const state = drive(game, [{ kind: 'tap-square', square: new Square(3, 3) }]);
    expect(state.selection.kind).toBe('none');
  });
});

describe('reduceInteraction — 成り', () => {
  it('成・不成が選べる手は問い合わせを出し、指してはいない', () => {
    // 先手の飛車 2四。2三 へ進むと成・不成の両方が合法。
    const game = new ShogiGame(Position.newBySFEN('4k4/9/9/7R1/9/9/9/9/4K4 b - 1')!);
    const state = drive(game, [
      { kind: 'tap-square', square: new Square(2, 4) },
      { kind: 'tap-square', square: new Square(2, 3) },
    ]);
    expect(state.pending).not.toBeNull();
    expect(game.ply, '選択中はまだ指していない').toBe(0);
  });

  it('「成る」を選ぶと成って着手される', () => {
    const game = new ShogiGame(Position.newBySFEN('4k4/9/9/7R1/9/9/9/9/4K4 b - 1')!);
    drive(game, [
      { kind: 'tap-square', square: new Square(2, 4) },
      { kind: 'tap-square', square: new Square(2, 3) },
      { kind: 'choose-promotion', promote: true },
    ]);
    expect(game.lastMove?.usi).toBe('2d2c+');
  });

  it('「不成」を選ぶとそのまま着手される', () => {
    const game = new ShogiGame(Position.newBySFEN('4k4/9/9/7R1/9/9/9/9/4K4 b - 1')!);
    drive(game, [
      { kind: 'tap-square', square: new Square(2, 4) },
      { kind: 'tap-square', square: new Square(2, 3) },
      { kind: 'choose-promotion', promote: false },
    ]);
    expect(game.lastMove?.usi).toBe('2d2c');
  });

  it('行き所のない駒は問わずに成って着手される', () => {
    // 先手の歩 5二 → 5一 は不成が非合法なので強制成り。
    const game = new ShogiGame(Position.newBySFEN('4k4/4P4/9/9/9/9/9/9/4K4 b - 1')!);
    const state = drive(game, [
      { kind: 'tap-square', square: new Square(5, 2) },
      { kind: 'tap-square', square: new Square(5, 1) },
    ]);
    expect(state.pending, '問い合わせは出ない').toBeNull();
    expect(game.lastMove?.usi).toBe('5b5a+');
  });
});

describe('reduceInteraction — 駒台から打つ', () => {
  it('持ち駒を選んで空きマスを叩くと打てる', () => {
    const game = new ShogiGame(Position.newBySFEN('4k4/9/9/9/9/9/9/9/4K4 b P 1')!);
    drive(game, [
      { kind: 'tap-hand', pieceType: PieceType.PAWN },
      { kind: 'tap-square', square: new Square(5, 5) },
    ]);
    expect(game.lastMove?.usi).toBe('P*5e');
  });

  it('持っていない駒は掴めない', () => {
    const game = new ShogiGame();
    const state = drive(game, [{ kind: 'tap-hand', pieceType: PieceType.ROOK }]);
    expect(state.selection.kind).toBe('none');
  });

  it('同じ持ち駒を二度叩くと選択解除', () => {
    const game = new ShogiGame(Position.newBySFEN('4k4/9/9/9/9/9/9/9/4K4 b P 1')!);
    const state = drive(game, [
      { kind: 'tap-hand', pieceType: PieceType.PAWN },
      { kind: 'tap-hand', pieceType: PieceType.PAWN },
    ]);
    expect(state.selection.kind).toBe('none');
  });

  it('二歩になるマスには打てない（選択は解除される）', () => {
    // 5筋に先手の歩がある状態で 5五 へ打とうとする。
    const game = new ShogiGame(Position.newBySFEN('4k4/9/9/9/9/9/4P4/9/4K4 b P 1')!);
    const state = drive(game, [
      { kind: 'tap-hand', pieceType: PieceType.PAWN },
      { kind: 'tap-square', square: new Square(5, 5) },
    ]);
    expect(game.ply, '二歩は指せない').toBe(0);
    expect(state.selection.kind).toBe('none');
  });
});

describe('reduceInteraction — cancel', () => {
  it('いつでも初期状態へ戻せる', () => {
    const game = new ShogiGame();
    const state = drive(game, [
      { kind: 'tap-square', square: new Square(7, 7) },
      { kind: 'cancel' },
    ]);
    expect(state).toEqual(INITIAL_INTERACTION);
  });
});
