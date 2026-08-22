import { useCallback, useEffect, useRef, useState } from 'react';
import { Color } from 'tsshogi';
import type { ShogiGame } from '../rules/game.js';
import { runEngineSearch, type RunningSearch } from '../ai/driver.js';
import { selectEngine } from '../ai/select-engine.js';
import type { ShogiEngine } from '../ai/engine.js';

/**
 * AI の手番を回すフック。エンジンの選択は**実行時**（`selectEngine`）で、
 * 思考は `runEngineSearch` がフレーム境界で刻む — 思考中も盤と操作は生きている。
 */

/** AI が 1 手に使う時間。スマホでも待たされ過ぎない範囲に切る。 */
const MOVE_BUDGET_MS = 1_200;

export interface EngineTurnOptions {
  readonly game: ShogiGame;
  /** AI が持つ手番。`null` なら人対人（AI を動かさない）。 */
  readonly aiColor: Color | null;
  /** AI が着手した後に呼ぶ（再描画のトリガ）。 */
  readonly onMoved: () => void;
}

export interface EngineTurnState {
  /** 思考中に読めている深さ。思考していなければ `null`。 */
  readonly thinkingDepth: number | null;
  /** エンジンの表示名（未初期化なら `null`）。 */
  readonly engineName: string | null;
  /** AI の思考を中断する（待った・投了・新規対局から呼ぶ）。 */
  readonly abort: () => void;
}

export function useEngineTurn({ game, aiColor, onMoved }: EngineTurnOptions): EngineTurnState {
  const [engine, setEngine] = useState<ShogiEngine | null>(null);
  const [thinkingDepth, setThinkingDepth] = useState<number | null>(null);
  const runningRef = useRef<RunningSearch | null>(null);

  // エンジンは 1 度だけ選ぶ。選択は実行時の capability プローブに委ねる。
  useEffect(() => {
    let disposed = false;
    void selectEngine().then((selected) => {
      if (disposed) selected.dispose();
      else setEngine(selected);
    });
    return () => {
      disposed = true;
    };
  }, []);

  const abort = useCallback(() => {
    runningRef.current?.cancel();
    runningRef.current = null;
    setThinkingDepth(null);
  }, []);

  const isAiTurn = aiColor !== null && !game.isOver && game.sideToMove === aiColor;

  useEffect(() => {
    if (engine === null || !isAiTurn || runningRef.current !== null) return;

    // 探索は現局面の SFEN だけを見る（千日手の判定は `ShogiGame` 側の責務）。
    setThinkingDepth(0);
    runningRef.current = runEngineSearch(
      engine,
      { sfen: game.sfen, moves: [], budgetMs: MOVE_BUDGET_MS },
      {
        onProgress: (progress) => setThinkingDepth(progress.depth),
        onDone: (result) => {
          runningRef.current = null;
          setThinkingDepth(null);
          // 合法手が無い＝詰み。`ShogiGame` が既に決着を持っているので何もしない。
          if (result.bestMove === null) return;
          const move = game.position.createMoveByUSI(result.bestMove);
          if (move !== null && game.play(move)) onMoved();
        },
      },
    );
    // 局面が動くたびに評価し直す（`game` は同一 identity なので revision を依存に入れる）。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [engine, isAiTurn, game, game.revision, onMoved]);

  // 手番が AI から人へ戻ったら（待った等）、走っている思考を止める。
  useEffect(() => {
    if (!isAiTurn) abort();
  }, [isAiTurn, abort]);

  return { thinkingDepth, engineName: engine?.displayName ?? null, abort };
}
