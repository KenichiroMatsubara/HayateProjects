import type { EngineProgress, EngineSearch, EngineSearchRequest, ShogiEngine } from './engine.js';

/**
 * 探索をフレーム境界で刻んで走らせるドライバ。**この 1 ファイルが「譲り方」を独占する**。
 *
 * ## 譲りは `setTimeout` のみ（`queueMicrotask` / 素の `Promise.then` は禁止）
 *
 * 埋め込み Hermes（ネイティブホスト）では:
 *
 * - `setTimeout` は `Promise.resolve().then(cb)` として shim され、Hermes は Promise ジョブを
 *   ホスト注入の `setImmediate` に流す。そのキューは**スナップショット交換方式**で
 *   `pump_frame` が 1 回だけ排出するため、排出中に積まれたコールバックは**次フレーム**に回る。
 *   → 自己連鎖する `setTimeout` は **1 vsync に 1 スライス**で、フレーム内で再入しない。
 * - 一方 `drainMicrotasks()` は**枯渇するまで**呼ばれる。
 *   → 自己連鎖する `queueMicrotask` / `Promise.then` は **1 フレーム内で無限に回り、端末をハングさせる**。
 *
 * ブラウザでは `setTimeout` は本物のタイマ（最短 ~4ms クランプ）になり、
 * 同じコードがそのまま「フレームを止めない」挙動になる。
 *
 * ## 時計は `Date.now` のみ
 *
 * 埋め込み Hermes に `performance` は無い（native prelude も注入しない）。
 */

/**
 * 1 スライスに使う時間。16.7ms のフレームから React の再描画とレイアウトに余白を残す。
 */
export const SLICE_BUDGET_MS = 6;

export interface EngineDriverCallbacks {
  /** 深さが進んだときだけ呼ぶ（毎スライス上げると再描画でフレーム予算を食う）。 */
  readonly onProgress?: (progress: Extract<EngineProgress, { kind: 'thinking' }>) => void;
  readonly onDone: (result: Extract<EngineProgress, { kind: 'done' }>) => void;
}

export interface RunningSearch {
  /** 探索を打ち切る。`onDone` は呼ばれない。 */
  cancel(): void;
}

/**
 * `engine` に 1 手考えさせる。スライスごとに `setTimeout` で譲るので、
 * 思考中も入力とフレームが生き続ける。
 */
export function runEngineSearch(
  engine: ShogiEngine,
  request: EngineSearchRequest,
  callbacks: EngineDriverCallbacks,
): RunningSearch {
  const search: EngineSearch = engine.search(request);
  const deadline = Date.now() + request.budgetMs;
  let cancelled = false;
  let reportedDepth = 0;

  const step = (): void => {
    if (cancelled) return;

    const progress = search.poll(SLICE_BUDGET_MS);
    if (progress.kind === 'done') {
      callbacks.onDone(progress);
      return;
    }

    if (Date.now() >= deadline) {
      // 時間切れ。打ち切って、その時点で最も深く読めた手を確定させる。
      search.cancel();
      const settled = search.poll(SLICE_BUDGET_MS);
      if (settled.kind === 'done') callbacks.onDone(settled);
      return;
    }

    if (progress.depth > reportedDepth) {
      reportedDepth = progress.depth;
      callbacks.onProgress?.(progress);
    }

    // 次のスライスへ。**setTimeout でなければならない**（上の注記）。
    setTimeout(step, 0);
  };

  setTimeout(step, 0);

  return {
    cancel: () => {
      cancelled = true;
      search.cancel();
    },
  };
}
