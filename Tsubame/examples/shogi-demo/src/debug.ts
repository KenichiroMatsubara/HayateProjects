import type { ShogiGame } from './rules/game.js';

/**
 * e2e 用のシーム。Hayate Renderer 経路では盤が canvas の中に閉じて DOM から見えないので、
 * 局面の主張はこのハンドル越しに行う（`?debug=1` のときだけ生やす）。
 * solid / react のデモが `data-*` 属性でホストの状態を晒しているのと同じ役割。
 */

/** グローバルに生やす名前。e2e から `window.__shogiDebug` で触る。 */
const DEBUG_GLOBAL = '__shogiDebug';

export interface ShogiDebugHandle {
  /** 現局面の SFEN。 */
  sfen(): string;
  /** 直前の指し手（USI）。無ければ `null`。 */
  lastMove(): string | null;
  /** 手数。 */
  ply(): number;
}

function debugEnabled(): boolean {
  const search = (globalThis as { location?: { search?: string } }).location?.search;
  if (typeof search !== 'string') return false;
  return new URLSearchParams(search).get('debug') === '1';
}

/** 現在の対局を e2e から観測できるようにする。`?debug=1` が無ければ何もしない。 */
export function publishDebugHandle(game: ShogiGame): void {
  if (!debugEnabled()) return;
  const handle: ShogiDebugHandle = {
    sfen: () => game.sfen,
    lastMove: () => game.lastMove?.usi ?? null,
    ply: () => game.ply,
  };
  (globalThis as unknown as Record<string, unknown>)[DEBUG_GLOBAL] = handle;
}
