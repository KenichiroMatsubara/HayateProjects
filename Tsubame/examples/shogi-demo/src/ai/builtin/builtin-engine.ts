import { Position } from 'tsshogi';
import type {
  EngineProgress,
  EngineSearch,
  EngineSearchRequest,
  ShogiEngine,
  ShogiEnginePlugin,
} from '../engine.js';
import { searchBestMove, type CancelToken, type SearchOutcome } from './search.js';

/**
 * 内蔵エンジン。**WASM も Worker も使わない純 TS** なので、ブラウザでも
 * 埋め込み Hermes（ネイティブホスト）でも同じコードがそのまま動く — マルチOSで
 * 挙動を揃える唯一の実装。強エンジンはこの上に**任意で**乗る。
 */

export const BUILTIN_ENGINE_ID = 'builtin';

/** 既定の探索深さ。tsshogi の合法手生成が重いので深くは掘らない。 */
const DEFAULT_MAX_DEPTH = 3;

class BuiltinSearch implements EngineSearch {
  private readonly generator: Generator<{ depth: number; nodes: number }, SearchOutcome, void>;
  private readonly token: CancelToken = { cancelled: false };
  private finished: EngineProgress | null = null;
  private lastTick = { depth: 0, nodes: 0 };

  constructor(request: EngineSearchRequest) {
    const position = positionOf(request);
    if (position === null) {
      this.finished = { kind: 'done', bestMove: null, scoreCp: 0, depth: 0, nodes: 0 };
      this.generator = (function* () {
        return { bestMove: null, scoreCp: 0, depth: 0, nodes: 0 };
      })();
      return;
    }
    this.generator = searchBestMove(position, request.maxDepth ?? DEFAULT_MAX_DEPTH, this.token);
  }

  poll(sliceBudgetMs: number): EngineProgress {
    if (this.finished !== null) return this.finished;

    // 時計は Date.now — 埋め込み Hermes に `performance` は無い
    // （`Torimi/bundle/src/native-prelude.ts` は shim していない）。
    const deadline = Date.now() + Math.max(1, sliceBudgetMs);
    while (Date.now() < deadline) {
      const step = this.generator.next();
      if (step.done) {
        const outcome = step.value;
        this.finished = {
          kind: 'done',
          bestMove: outcome.bestMove?.usi ?? null,
          scoreCp: outcome.scoreCp,
          depth: outcome.depth,
          nodes: outcome.nodes,
        };
        return this.finished;
      }
      this.lastTick = step.value;
    }
    return { kind: 'thinking', depth: this.lastTick.depth, nodes: this.lastTick.nodes };
  }

  cancel(): void {
    this.token.cancelled = true;
    // 打ち切りは次の yield の後で効くので、ここで残りを回し切って結果を確定させる。
    if (this.finished === null) {
      let step = this.generator.next();
      while (!step.done) step = this.generator.next();
      const outcome = step.value;
      this.finished = {
        kind: 'done',
        bestMove: outcome.bestMove?.usi ?? null,
        scoreCp: outcome.scoreCp,
        depth: outcome.depth,
        nodes: outcome.nodes,
      };
    }
  }
}

class BuiltinEngine implements ShogiEngine {
  readonly id = BUILTIN_ENGINE_ID;
  readonly displayName = '内蔵AI';

  search(request: EngineSearchRequest): EngineSearch {
    return new BuiltinSearch(request);
  }

  dispose(): void {
    // 保持する資源は無い。
  }
}

/** SFEN + USI 指し手列から探索の開始局面を組む。 */
function positionOf(request: EngineSearchRequest): Position | null {
  const position = Position.newBySFEN(request.sfen);
  if (position === null) return null;
  for (const usi of request.moves) {
    const move = position.createMoveByUSI(usi);
    if (move === null || !position.doMove(move)) return null;
  }
  return position;
}

/** 常に使える既定プラグイン。選択の最後の砦なので `isAvailable` は常に真。 */
export const builtinEnginePlugin: ShogiEnginePlugin = {
  id: BUILTIN_ENGINE_ID,
  isAvailable: () => true,
  create: () => Promise.resolve(new BuiltinEngine()),
};
