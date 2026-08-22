import { describe, expect, it } from 'vitest';
import { Color, PieceType, Position, Square } from 'tsshogi';
import { ShogiGame, listLegalMoves } from './game.js';

/**
 * ルールの正否は tsshogi が正本なので、ここで見るのは**自分のラッパが
 * tsshogi の判定を素通しできているか**（配線の確認）だけ。禁じ手の網羅テストを
 * 自前で持つと二重管理になる。
 */

/** SFEN から局面を作る（テスト用。壊れた SFEN は即座に失敗させる）。 */
function positionOf(sfen: string): Position {
  const position = Position.newBySFEN(sfen);
  expect(position, `invalid SFEN: ${sfen}`).not.toBeNull();
  return position!;
}

describe('ShogiGame — 初期局面', () => {
  it('平手の初期局面から始まり、合法手は30手ある', () => {
    const game = new ShogiGame();
    expect(game.sfen).toBe('lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1');
    expect(game.sideToMove).toBe(Color.BLACK);
    expect(game.inCheck).toBe(false);
    expect(game.isOver).toBe(false);
    // 平手初期局面の合法手は 30（歩18 + 香4 + 桂2 + 銀4 + 金2… の総和）。
    expect(game.listLegalMoves()).toHaveLength(30);
  });

  it('7六歩を指すと手番が移り、直前手がハイライト用に取れる', () => {
    const game = new ShogiGame();
    const move = game.createMove(new Square(7, 7), new Square(7, 6), false);
    expect(move).not.toBeNull();
    expect(game.play(move!)).toBe(true);

    expect(game.sideToMove).toBe(Color.WHITE);
    expect(game.lastMove?.usi).toBe('7g7f');
    expect(game.revision).toBe(1);
  });

  it('非合法な手は createMove が null を返す（2マス進む歩）', () => {
    const game = new ShogiGame();
    expect(game.createMove(new Square(7, 7), new Square(7, 5), false)).toBeNull();
  });
});

describe('ShogiGame — 禁じ手が tsshogi 経由で弾かれる', () => {
  it('二歩: 自分の歩がある筋には打てないが、と金しかない筋には打てる', () => {
    // 5筋に先手の歩、4筋に先手のと金。先手は歩を持っている。
    const withPawn = positionOf('4k4/9/9/9/9/9/4P4/9/4K4 b P 1');
    expect(withPawn.createMove(PieceType.PAWN, new Square(5, 5))).not.toBeNull();
    const doubled = withPawn.createMove(PieceType.PAWN, new Square(5, 5))!;
    expect(withPawn.isValidMove(doubled), '5筋は二歩').toBe(false);

    const withTokin = positionOf('4k4/9/9/9/9/9/5+P3/9/4K4 b P 1');
    const onTokinFile = withTokin.createMove(PieceType.PAWN, new Square(4, 5))!;
    expect(withTokin.isValidMove(onTokinFile), 'と金しかない筋は二歩ではない').toBe(true);
  });

  it('行き所のない駒: 1段目へ進む歩は成りが強制される', () => {
    // 先手の歩が 5二 にいる。5一 へ進むと不成は行き所がない。
    const position = positionOf('4k4/4P4/9/9/9/9/9/9/4K4 b - 1');
    const game = new ShogiGame(position);
    expect(game.promotionChoice(new Square(5, 2), new Square(5, 1))).toBe('forced');
  });

  it('成り: 3段目に入る手は成・不成の両方が選べる', () => {
    const position = positionOf('4k4/9/9/4P4/9/9/9/9/4K4 b - 1');
    const game = new ShogiGame(position);
    expect(game.promotionChoice(new Square(5, 4), new Square(5, 3))).toBe('optional');
  });

  it('自陣での歩の前進は成れない', () => {
    const game = new ShogiGame();
    expect(game.promotionChoice(new Square(7, 7), new Square(7, 6))).toBe('none');
  });

  it('ピンされた駒は筋から外れられないが、筋に沿ってなら動ける', () => {
    // 後手の飛車 5一 — 先手の金 5八 — 先手玉 5九 が 5筋に一直線。金はピンされている。
    const position = positionOf('4r4/9/9/9/9/9/9/4G4/4K4 b - 1');
    const gold = new Square(5, 8);
    const goldMoves = listLegalMoves(position).filter((move) => gold.equals(move.from as Square));

    expect(
      goldMoves.every((move) => move.to.file === 5),
      '金がピン筋から外れる手は無い',
    ).toBe(true);
    expect(
      goldMoves.some((move) => move.to.equals(new Square(5, 7))),
      '筋に沿って前進はできる',
    ).toBe(true);
  });
});

describe('ShogiGame — 決着', () => {
  it('投了すると相手の勝ちになり、以後は指せない', () => {
    const game = new ShogiGame();
    expect(game.resign()).toBe(true);
    expect(game.outcome).toEqual({ kind: 'resign', winner: Color.WHITE });
    expect(game.isOver).toBe(true);

    const move = game.createMove(new Square(7, 7), new Square(7, 6), false);
    expect(move).not.toBeNull();
    expect(game.play(move!), '決着後は着手できない').toBe(false);
  });

  it('待ったは指定手数ぶん戻し、決着も取り消す', () => {
    const game = new ShogiGame();
    const first = game.createMove(new Square(7, 7), new Square(7, 6), false)!;
    game.play(first);
    const second = game.createMove(new Square(3, 3), new Square(3, 4), false)!;
    game.play(second);
    expect(game.ply).toBe(2);

    expect(game.undo(2)).toBe(true);
    expect(game.ply).toBe(0);
    expect(game.sideToMove).toBe(Color.BLACK);
    expect(game.undo(2), '初期局面からは戻せない').toBe(false);
  });
});
