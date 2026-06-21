import type { ElementId, ElementKind, Unsubscribe } from '@tsubame/renderer-protocol';

/**
 * Hayate / DOM 要素（`ElementId`）に紐づく Solid のホストハンドル。
 *
 * 正準のツリー構造は Hayate（Canvas）またはブラウザ DOM（DOM Renderer）側に存在する。
 * このオブジェクトは `solid-js/universal` のツリー走査を満たし、リスナの
 * unsubscribe のみを保持する。テキスト内容はここではなくバックエンド要素側に置く
 * （ADR-0063）。
 */
export interface TsubameNode {
  readonly id: ElementId;
  readonly elementKind: ElementKind;
  parent: TsubameNode | null;
  readonly children: TsubameNode[];
  readonly events: Map<string, Unsubscribe>;
}

/** @deprecated Use {@link TsubameNode}. */
export type ElementNode = TsubameNode;

export function createElementNode(id: ElementId, elementKind: ElementKind): TsubameNode {
  return {
    id,
    elementKind,
    parent: null,
    children: [],
    events: new Map(),
  };
}
