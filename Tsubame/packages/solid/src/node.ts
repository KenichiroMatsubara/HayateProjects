import type { ElementId, ElementKind, Unsubscribe } from '@tsubame/renderer-protocol';

/**
 * Solid host handle bound to a Hayate / DOM element (`ElementId`).
 *
 * Document Tree structure lives in Hayate (Canvas) or the browser DOM (DOM
 * Renderer). This object satisfies `solid-js/universal` tree walks and holds
 * listener unsubscribes only. Text content lives on the backend element, not
 * here (ADR-0063).
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
