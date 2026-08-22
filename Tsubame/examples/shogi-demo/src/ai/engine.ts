/**
 * AI のシーム。**`poll` 1 本**に畳むことで、同期の内蔵エンジンと非同期の
 * Worker / WASM エンジンが同じ形になる。
 *
 * 局面の受け渡しは **SFEN と USI** で行う。これは強い将棋エンジン（USI プロトコル）
 * との境界そのものなので、内蔵エンジンを同じ語彙に合わせておけば、後から
 * 外部エンジンを足しても呼ぶ側は変わらない。
 */

export interface EngineSearchRequest {
  /** 開始局面。 */
  readonly sfen: string;
  /** 開始局面からの指し手（USI）。 */
  readonly moves: readonly string[];
  /** この 1 手に使ってよい総時間。 */
  readonly budgetMs: number;
  /** 探索の深さ上限。省略時はエンジンの既定。 */
  readonly maxDepth?: number;
}

export type EngineProgress =
  | {
      readonly kind: 'thinking';
      readonly depth: number;
      readonly nodes: number;
    }
  | {
      readonly kind: 'done';
      /** 最善手（USI）。合法手が無ければ `null`。 */
      readonly bestMove: string | null;
      /** 手番側から見た評価値（歩＝100 換算）。 */
      readonly scoreCp: number;
      readonly depth: number;
      readonly nodes: number;
    };

export interface EngineSearch {
  /**
   * 進められるだけ進めて現在の状態を返す。
   *
   * 同期エンジンは `sliceBudgetMs` を**壁時計で**使い切って自発的に譲り、
   * 非同期エンジン（Worker / WASM）は何もせず現在の状態だけを返す。
   * `'done'` を返したあとは呼ばれない。
   */
  poll(sliceBudgetMs: number): EngineProgress;
  /** 探索を打ち切る。以後の `poll` は `'done'` を返す。 */
  cancel(): void;
}

export interface ShogiEngine {
  readonly id: string;
  readonly displayName: string;
  search(request: EngineSearchRequest): EngineSearch;
  dispose(): void;
}

/**
 * エンジンの実装候補。**実行時**の capability プローブで選ぶ（ビルド時分岐は置かない）。
 * `@torimi/bundle` が `__hayateHost` の有無で分岐するのと同じ流儀（ADR-0008 §4）。
 */
export interface ShogiEnginePlugin {
  readonly id: string;
  /** この実行環境で使えるか。 */
  isAvailable(): boolean;
  create(): Promise<ShogiEngine>;
}
