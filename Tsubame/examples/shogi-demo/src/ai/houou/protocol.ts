/**
 * Worker との往復で流す形。**この 1 ファイルが両岸の唯一の契約**で、
 * `worker.ts`（wasm 側）と `houou-engine.ts`（UI 側）はここだけを共有する。
 *
 * `info` / `done` の中身は Rust 側が JSON 文字列で組んでいる
 * （`Houou/crates/houou-wasm/src/lib.rs`）。形を変えるときは両方を直すこと。
 */

/** 探索 1 段ぶんの進捗。 */
export interface HououInfo {
  readonly depth: number;
  /** 手番側から見た評価値（歩＝100 換算）。 */
  readonly scoreCp: number;
  readonly nodes: number;
  readonly nps: number;
  readonly elapsedMs: number;
  /** 詰みが見えていれば「あと何手で詰むか」。手番側が詰ますなら正。 */
  readonly mate: number | null;
  /** 読み筋（USI）。先頭が現時点の最善手。 */
  readonly pv: readonly string[];
}

/** 探索の結末。 */
export interface HououResult {
  /** 最善手（USI）。合法手が無ければ `null`。 */
  readonly bestMove: string | null;
  readonly ponder: string | null;
  readonly scoreCp: number;
  readonly depth: number;
  readonly nodes: number;
  readonly elapsedMs: number;
  readonly mate: number | null;
}

export type WorkerRequest =
  | { readonly kind: 'init'; readonly hashMb: number }
  | { readonly kind: 'newGame' }
  | {
      readonly kind: 'search';
      /** 応答をどの探索のものか見分ける番号。打ち切った探索の遅れた応答を捨てるために要る。 */
      readonly id: number;
      readonly sfen: string;
      readonly moves: readonly string[];
      readonly budgetMs: number;
      /** 0 なら深さ無制限（時間だけで打ち切る）。 */
      readonly maxDepth: number;
    };

export type WorkerResponse =
  | { readonly kind: 'ready'; readonly version: string }
  | { readonly kind: 'info'; readonly id: number; readonly info: HououInfo }
  | { readonly kind: 'done'; readonly id: number; readonly result: HououResult }
  | { readonly kind: 'failed'; readonly id: number; readonly message: string };
