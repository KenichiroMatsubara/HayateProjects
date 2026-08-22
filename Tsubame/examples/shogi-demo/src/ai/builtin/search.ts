import { Color, Move, Position, type ImmutablePosition } from 'tsshogi';
import { listLegalMoves } from '../../rules/game.js';
import { evaluate, pieceValue } from './evaluate.js';

/**
 * 反復深化 alpha-beta。**ジェネレータとして書く**のが要点で、`yield` のたびに
 * 呼び出し側へ制御が戻るので、探索をフレーム境界で好きなだけ刻める。
 * 手続きを自前の継続マシンに畳み直さずに済み、探索そのものは素直な再帰のまま読める。
 *
 * 深さが浅いのは tsshogi の合法手生成が 1 局面あたり 0.6ms 程度かかるため
 * （盤上の自駒 × 全マスを `isValidMove` で漉す）。強さより「全ホストで同じコードが
 * フレームを止めずに動く」ことを採る。
 */

/** 詰みのスコア。深さで割り引いて「早い詰み」を優先させる。 */
const MATE = 100_000;
/** 評価値の上限（alpha-beta の初期窓）。 */
const INFINITY = 1_000_000;

export interface SearchOutcome {
  readonly bestMove: Move | null;
  readonly scoreCp: number;
  readonly depth: number;
  readonly nodes: number;
}

/** 探索の進捗。`nodes` は生成した局面数。 */
export interface SearchTick {
  readonly depth: number;
  readonly nodes: number;
}

/** 探索の打ち切り札。呼び出し側が `cancelled` を立てると、次の `yield` の後で気付く。 */
export interface CancelToken {
  cancelled: boolean;
}

/** 探索中に更新される可変カウンタ。 */
interface Counters {
  nodes: number;
  depth: number;
  readonly token: CancelToken;
}

/**
 * 反復深化で `maxDepth` まで掘る。各深さの完走ごとに最善手を更新するので、
 * 途中で打ち切っても**その時点で最も深く読めた手**が残る。
 */
export function* searchBestMove(
  root: ImmutablePosition,
  maxDepth: number,
  token: CancelToken,
): Generator<SearchTick, SearchOutcome, void> {
  const position = root.clone();
  const me = position.color;
  const counters: Counters = { nodes: 0, depth: 0, token };

  let best: Move | null = null;
  let bestScore = -INFINITY;

  const rootMoves = orderMoves(position, listLegalMoves(position));
  if (rootMoves.length === 0) {
    return { bestMove: null, scoreCp: -MATE, depth: 0, nodes: 0 };
  }
  best = rootMoves[0]!;

  for (let depth = 1; depth <= maxDepth; depth += 1) {
    counters.depth = depth;
    let alpha = -INFINITY;
    let localBest: Move | null = null;

    for (const move of rootMoves) {
      position.doMove(move, { ignoreValidation: true });
      const score = -(yield* negamax(position, depth - 1, -INFINITY, -alpha, me, counters));
      position.undoMove(move);

      if (score > alpha) {
        alpha = score;
        localBest = move;
      }
      yield { depth, nodes: counters.nodes };
      if (counters.token.cancelled) break;
    }

    if (localBest !== null) {
      best = localBest;
      bestScore = alpha;
      // 次の深さで良い手から読めるよう、最善手を先頭へ持ってくる。
      rootMoves.splice(rootMoves.indexOf(localBest), 1);
      rootMoves.unshift(localBest);
    }
    if (counters.token.cancelled) break;
  }

  return { bestMove: best, scoreCp: bestScore, depth: counters.depth, nodes: counters.nodes };
}

/** 手番側から見た negamax。`me` は評価の基準となる側（根の手番）。 */
function* negamax(
  position: Position,
  depth: number,
  alpha: number,
  beta: number,
  me: Color,
  counters: Counters,
): Generator<SearchTick, number, void> {
  counters.nodes += 1;
  if (depth <= 0) {
    // 葉。手番側から見た値にするため、根の手番と一致しなければ符号を反転する。
    return position.color === me
      ? evaluate(position, me)
      : -evaluate(position, me);
  }

  const moves = listLegalMoves(position);
  // 合法手が無い = 詰み。深いほど軽く見て「早く詰ます手」を選ばせる。
  if (moves.length === 0) return -MATE + counters.nodes % 1000;

  // 1 局面ぶんの生成が終わったので譲る機会を作る（フレームを止めない）。
  yield { depth: counters.depth, nodes: counters.nodes };
  if (counters.token.cancelled) return alpha;

  let value = -INFINITY;
  for (const move of orderMoves(position, moves)) {
    position.doMove(move, { ignoreValidation: true });
    const score = -(yield* negamax(position, depth - 1, -beta, -alpha, me, counters));
    position.undoMove(move);

    if (score > value) value = score;
    if (value > alpha) alpha = value;
    if (alpha >= beta) break; // beta カット
    if (counters.token.cancelled) break;
  }
  return value;
}

/**
 * 指し手の並べ替え。**駒を取る手を、取る駒の価値が高い順に前へ**出すだけの
 * 素朴な順序付けだが、alpha-beta の枝刈りはこれだけでもよく効く。
 */
function orderMoves(position: Position | ImmutablePosition, moves: readonly Move[]): Move[] {
  return [...moves].sort((a, b) => captureGain(position, b) - captureGain(position, a));
}

function captureGain(position: Position | ImmutablePosition, move: Move): number {
  const captured = position.board.at(move.to);
  const base = captured === null ? 0 : pieceValue(captured.type);
  return move.promote ? base + 200 : base;
}
