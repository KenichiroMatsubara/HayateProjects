import type { ElementId, ElementKind, Unsubscribe } from '@tsubame/renderer-protocol';

/**
 * Solid host handle bound to a Hayate / DOM element (`ElementId`).
 *
 * Document Tree structure lives in Hayate (Canvas) or the browser DOM (DOM
 * Renderer). This object satisfies `solid-js/universal` tree walks and holds
 * listener unsubscribes only.
 */
export interface TsubameNode {
  readonly id: ElementId;
  readonly elementKind: ElementKind;
  parent: TsubameNode | null;
  readonly children: TsubameNode[];
  readonly events: Map<string, Unsubscribe>;
  /** Latest text for `text` elements (Solid `replaceText`). */
  text: string;
}

/** @deprecated Use {@link TsubameNode}. */
export type ElementNode = TsubameNode;

export function createElementNode(
  id: ElementId,
  elementKind: ElementKind,
  text = '',
): TsubameNode {
  return {
    id,
    elementKind,
    parent: null,
    children: [],
    events: new Map(),
    text,
  };
}

/** `<text>…</text>`: Solid text child collapsed into parent (DOM `<span>` model). */
export function isTextInTextCollapse(parent: TsubameNode, child: TsubameNode): boolean {
  return parent.elementKind === 'text' && child.elementKind === 'text';
}
