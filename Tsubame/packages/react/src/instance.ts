import type { ElementId, ElementKind, Unsubscribe } from '@tsubame/renderer-protocol';

/**
 * `@tsubame/react` のホスト instance は**構造ゼロ**である（ADR-0010 / ADR-0062）。
 *
 * React の Fiber tree が構造の記帳（親子・兄弟）を担うため、ホスト instance に
 * `parent` / `children` を持たせる必要がない。instance が保持するのは要素 id・kind と、
 * 自身に登録したイベントリスナの解除関数だけである（リスナ差し替え用の最小情報）。
 * subtree の構造片付けは `IRenderer.removeChild` に委ね、各 instance のリスナ解除は
 * reconciler の `detachDeletedInstance` で行う（構造を辿らない）。
 */
export interface TsubameInstance {
  readonly id: ElementId;
  readonly kind: ElementKind;
  /** prop 名 → 解除関数。同名イベント prop の差し替え時に古い購読を解除する。 */
  readonly listeners: Map<string, Unsubscribe>;
}

/**
 * テキストノードも Hayate の `text` element である（ADR-0058）。リスナを持たないため
 * 構造は id だけで足りる。
 */
export interface TsubameTextInstance {
  readonly id: ElementId;
}

export function createInstance(id: ElementId, kind: ElementKind): TsubameInstance {
  return { id, kind, listeners: new Map() };
}
