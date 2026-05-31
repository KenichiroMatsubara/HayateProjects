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

/**
 * text ノード。IRenderer の element は作らず、親 ElementNode の `setText` を
 * 通じてテキストを届ける仮想ノード。
 *
 * Solid の universal renderer は JSX 文字コンテンツを別 textNode として扱うが、
 * Tsubame の設計では「text/button element 1 つがスタイルとテキストを両方持つ」
 * ため、textNode は IRenderer ツリーに追加せず親の setText で集約する。
 */
export interface TextNode {
  readonly kind: 'text';
  /** shadow ツリー内での同一性確認用の仮想 ID。IRenderer には登録しない。 */
  readonly id: ElementId;
  parent: ElementNode | null;
  text: string;
}

export type TsubameNode = ElementNode | TextNode;

/** 仮想 TextNode 用の連番（負数）。IRenderer の ElementId と衝突しない。 */
let _nextVirtualId = -1;

export function createElementNode(id: ElementId): ElementNode {
  return { kind: 'element', id, parent: null, children: [], events: new Map() };
}

export function createTextShadowNode(text: string): TextNode {
  return { kind: 'text', id: _nextVirtualId-- as ElementId, parent: null, text };
}
