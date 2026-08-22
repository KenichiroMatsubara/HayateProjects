import { useCallback, useMemo, useReducer, useRef, useState } from 'react';
import { Color, PieceType, Square, reverseColor } from 'tsshogi';
import { ShogiGame } from './rules/game.js';
import type { BoardPainterState } from './paint/board-painter.js';
import { Board } from './ui/Board.js';
import { Hand } from './ui/Hand.js';
import { StatusBar } from './ui/StatusBar.js';
import { Controls } from './ui/Controls.js';
import { PromotionPrompt } from './ui/PromotionPrompt.js';
import { useEngineTurn } from './ui/use-engine.js';
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
  // AI の手番。既定は後手を AI が持つ（人が先手で指し始められる）。`null` で人対人。
  // `?opponent=human` で人対人から始められる（`?renderer=` と同じくクエリで初期状態を選ぶ）。
  const [aiColor, setAiColor] = useState<Color | null>(initialAiColor);
  const [, redraw] = useReducer((tick: number) => tick + 1, 0);
  const gameRef = useRef(game);
  gameRef.current = game;

  const commit = useCallback(() => {
    redraw();
  }, []);

  const { thinkingDepth, abort } = useEngineTurn({ game, aiColor, onMoved: commit });
  const busy = thinkingDepth !== null;

  const dispatch = useCallback(
    (action: Parameters<typeof reduceInteraction>[2]) => {
      const current = gameRef.current;
      // AI の手番中は盤を受け付けない（局面が動いている最中に触らせない）。
      if (aiColor !== null && current.sideToMove === aiColor) return;
      const result = reduceInteraction(current, interaction, action);
      setInteraction(result.state);
      if (result.move !== null && current.play(result.move)) commit();
    },
    [interaction, commit, aiColor],
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
    abort();
    setGame(new ShogiGame());
    setInteraction(INITIAL_INTERACTION);
    commit();
  }, [commit, abort]);

  const undo = useCallback(() => {
    abort();
    // AI 対局では 1 往復（自分と相手）を戻す。人対人なら 1 手。
    if (gameRef.current.undo(aiColor === null ? 1 : 2)) {
      setInteraction(INITIAL_INTERACTION);
      commit();
    }
  }, [commit, abort, aiColor]);

  const resign = useCallback(() => {
    abort();
    if (gameRef.current.resign()) {
      setInteraction(INITIAL_INTERACTION);
      commit();
    }
  }, [commit, abort]);

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
        revision={game.revision}
        active={!game.isOver && game.sideToMove === topColor}
        selected={game.sideToMove === topColor ? selectedHandPiece : null}
        flipped={flipped}
        onTapPiece={onTapHand}
      />

      <Board painterState={painterState} onTapSquare={onTapSquare} />

      <Hand
        color={bottomColor}
        hand={game.position.hand(bottomColor)}
        revision={game.revision}
        active={!game.isOver && game.sideToMove === bottomColor}
        selected={game.sideToMove === bottomColor ? selectedHandPiece : null}
        flipped={flipped}
        onTapPiece={onTapHand}
      />

      <StatusBar
        sideToMove={game.sideToMove}
        ply={game.ply}
        inCheck={game.inCheck}
        outcome={game.outcome}
        thinkingDepth={thinkingDepth}
      />

      <Controls
        busy={busy}
        canUndo={game.ply > 0}
        isOver={game.isOver}
        onNewGame={newGame}
        onUndo={undo}
        onResign={resign}
        onFlip={() => setFlipped((value) => !value)}
        opponent={aiColor === null ? 'human' : 'ai'}
        onToggleOpponent={() => {
          abort();
          setAiColor((color) => (color === null ? Color.WHITE : null));
        }}
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

/**
 * 起動時に AI が持つ手番。`?opponent=human` なら人対人（`null`）、既定は AI が後手。
 * e2e が盤の操作を決定的に確かめられるようにする入口でもある。
 */
const initialAiColor: Color | null = (() => {
  const search = (globalThis as { location?: { search?: string } }).location?.search;
  if (typeof search === 'string' && new URLSearchParams(search).get('opponent') === 'human') {
    return null;
  }
  return Color.WHITE;
})();

/** 手番側の玉のマス index。見つからなければ -1（詰将棋の局面など）。 */
function kingIndex(game: ShogiGame): number {
  for (const square of game.position.board.listNonEmptySquares()) {
    const piece = game.position.board.at(square);
    if (piece?.type === PieceType.KING && piece.color === game.sideToMove) return square.index;
  }
  return -1;
}
