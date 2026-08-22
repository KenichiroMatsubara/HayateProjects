import { useCallback, useMemo, useReducer, useRef, useState } from 'react';
import { Color, PieceType, Square, reverseColor } from 'tsshogi';
import { ShogiGame } from './rules/game.js';
import type { BoardPainterState } from './paint/board-painter.js';
import { Board } from './ui/Board.js';
import { Hand } from './ui/Hand.js';
import { StatusBar } from './ui/StatusBar.js';
import { Controls } from './ui/Controls.js';
import { PromotionPrompt } from './ui/PromotionPrompt.js';
import {
  INITIAL_INTERACTION,
  reduceInteraction,
  selectionSource,
  type InteractionState,
} from './ui/use-game.js';
import { publishDebugHandle } from './debug.js';
import * as S from './ui/styles.js';

/**
 * 将棋GUI。局面は `ShogiGame`（tsshogi のラッパ）が所有し、React state には載せない
 * — 盤はその場更新されるので、再描画は版数（`revision`）で駆動する。Sketch デモが
 * `SketchDocument` を state 外に置いているのと同じ形。
 */
export function App() {
  const [game, setGame] = useState(() => new ShogiGame());
  const [interaction, setInteraction] = useState<InteractionState>(INITIAL_INTERACTION);
  const [flipped, setFlipped] = useState(false);
  const [, redraw] = useReducer((tick: number) => tick + 1, 0);
  const gameRef = useRef(game);
  gameRef.current = game;

  const commit = useCallback(() => {
    redraw();
  }, []);

  const dispatch = useCallback(
    (action: Parameters<typeof reduceInteraction>[2]) => {
      const current = gameRef.current;
      const result = reduceInteraction(current, interaction, action);
      setInteraction(result.state);
      if (result.move !== null && current.play(result.move)) commit();
    },
    [interaction, commit],
  );

  const onTapSquare = useCallback(
    (square: Square) => dispatch({ kind: 'tap-square', square }),
    [dispatch],
  );
  const onTapHand = useCallback(
    (pieceType: PieceType) => dispatch({ kind: 'tap-hand', pieceType }),
    [dispatch],
  );

  const newGame = useCallback(() => {
    setGame(new ShogiGame());
    setInteraction(INITIAL_INTERACTION);
    commit();
  }, [commit]);

  const undo = useCallback(() => {
    // 人対人なので 1 手ずつ戻す（AI 対局を足したら 2 手に変える）。
    if (gameRef.current.undo(1)) {
      setInteraction(INITIAL_INTERACTION);
      commit();
    }
  }, [commit]);

  const resign = useCallback(() => {
    if (gameRef.current.resign()) {
      setInteraction(INITIAL_INTERACTION);
      commit();
    }
  }, [commit]);

  const source = selectionSource(interaction.selection);
  const targets = useMemo(
    () => (source === null ? [] : game.listTargets(source).map((square) => square.index)),
    // 局面が動けば候補も変わるので revision を依存に入れる（position は同一 identity のまま）。
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [game, game.revision, source],
  );

  const painterState: BoardPainterState = {
    position: game.position,
    revision: game.revision,
    selected: source instanceof Square ? source.index : -1,
    targets,
    lastFrom: game.lastMove?.from instanceof Square ? game.lastMove.from.index : -1,
    lastTo: game.lastMove?.to.index ?? -1,
    checkedKing: game.inCheck ? kingIndex(game) : -1,
    flipped,
  };

  publishDebugHandle(game);

  const selectedHandPiece =
    interaction.selection.kind === 'hand' ? interaction.selection.pieceType : null;
  // 盤を反転すると手前に来る側が入れ替わるので、駒台の上下も入れ替える。
  const topColor = flipped ? Color.BLACK : Color.WHITE;
  const bottomColor = reverseColor(topColor);

  return (
    <view style={S.shell}>
      <Hand
        color={topColor}
        hand={game.position.hand(topColor)}
        active={!game.isOver && game.sideToMove === topColor}
        selected={game.sideToMove === topColor ? selectedHandPiece : null}
        onTapPiece={onTapHand}
      />

      <Board position={game.position} painterState={painterState} onTapSquare={onTapSquare} />

      <Hand
        color={bottomColor}
        hand={game.position.hand(bottomColor)}
        active={!game.isOver && game.sideToMove === bottomColor}
        selected={game.sideToMove === bottomColor ? selectedHandPiece : null}
        onTapPiece={onTapHand}
      />

      <StatusBar
        sideToMove={game.sideToMove}
        ply={game.ply}
        inCheck={game.inCheck}
        outcome={game.outcome}
        thinkingDepth={null}
      />

      <Controls
        busy={false}
        canUndo={game.ply > 0}
        isOver={game.isOver}
        onNewGame={newGame}
        onUndo={undo}
        onResign={resign}
        onFlip={() => setFlipped((value) => !value)}
      />

      {interaction.pending !== null ? (
        <PromotionPrompt
          onChoose={(promote) => dispatch({ kind: 'choose-promotion', promote })}
          onCancel={() => dispatch({ kind: 'cancel' })}
        />
      ) : null}
    </view>
  );
}

/** 手番側の玉のマス index。見つからなければ -1（詰将棋の局面など）。 */
function kingIndex(game: ShogiGame): number {
  for (const square of game.position.board.listNonEmptySquares()) {
    const piece = game.position.board.at(square);
    if (piece?.type === PieceType.KING && piece.color === game.sideToMove) return square.index;
  }
  return -1;
}
