import type { ElementId, Unsubscribe } from '@tsubame/renderer-protocol';

/**
 * tsubame-solid が保持する shadow ノード。
 *
 * solid-js/universal はツリー走査（getParentNode / getFirstChild /
 * getNextSibling）と text ノード判定を Renderer 側に要求するが、Tsubame の
 * {@link IRenderer} はこれらを公開しない（mutation のみの境界）。そのため
 * Adapter 側で軽量なツリー構造を保持し、{@link ElementId} と対応付ける。
 */
export interface ElementNode {
  readonly kind: 'element';
  readonly id: ElementId;
  parent: ElementNode | null;
  readonly children: TsubameNode[];
  /** prop 名（onClick 等）→ 購読解除関数。 */
  readonly events: Map<string, Unsubscribe>;
}

/** text ノード。Tsubame では `text` element 1 つに対応する。 */
export interface TextNode {
  readonly kind: 'text';
  readonly id: ElementId;
  parent: ElementNode | null;
  text: string;
}

export type TsubameNode = ElementNode | TextNode;

export function createElementNode(id: ElementId): ElementNode {
  return { kind: 'element', id, parent: null, children: [], events: new Map() };
}

export function createTextShadowNode(id: ElementId, text: string): TextNode {
  return { kind: 'text', id, parent: null, text };
}
