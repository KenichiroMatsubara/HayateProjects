import { createRenderer } from 'solid-js/universal';
import type { ElementKind, EventHandler, StylePatch } from '@tsubame/renderer-protocol';
import { activeRenderer } from './active-renderer.js';
import {
  createElementNode,
  createTextShadowNode,
  type ElementNode,
  type TextNode,
  type TsubameNode,
} from './node.js';
import { EVENT_PROP } from './events.js';

/** 当該ノードと配下サブツリーのイベント購読をすべて解除する。 */
function disposeEvents(node: TsubameNode): void {
  if (node.kind !== 'element') return;
  for (const unsub of node.events.values()) unsub();
  node.events.clear();
  for (const child of node.children) disposeEvents(child);
}

/**
 * 親 ElementNode の text 系子ノードを走査してテキストを連結し、
 * IRenderer.setText を呼んで親 element のテキスト内容を更新する。
 *
 * Solid は `<text>Hello {name()}</text>` を
 *   TextNode("Hello ") + TextNode(name())
 * という複数の TextNode として扱う。これらは IRenderer には追加されず、
 * ここで連結されて setText に渡される。
 */
function refreshText(parent: ElementNode): void {
  const text = parent.children
    .filter((n): n is TextNode => n.kind === 'text')
    .map((n) => n.text)
    .join('');
  activeRenderer().setText(parent.id, text);
}

/**
 * children 配列の from 以降にある最初の ElementNode を返す。
 * ElementNode を IRenderer に挿入するとき、anchor が TextNode（IRenderer に
 * 存在しない仮想ノード）の場合に実 anchor を探すために使う。
 */
function nextElementSibling(parent: ElementNode, from: TsubameNode): ElementNode | undefined {
  const start = parent.children.indexOf(from);
  for (let i = start + 1; i < parent.children.length; i++) {
    const n = parent.children[i];
    if (n?.kind === 'element') return n;
  }
  return undefined;
}

const {
  render,
  effect,
  memo,
  createComponent,
  createElement,
  createTextNode,
  insertNode,
  insert,
  spread,
  setProp,
  mergeProps,
} = createRenderer<TsubameNode>({
  createElement(tag: string): TsubameNode {
    return createElementNode(activeRenderer().createElement(tag as ElementKind));
  },

  /**
   * Solid のテキストノードを仮想ノードとして作成する。
   * IRenderer には element を作らず、親への setText 経由でのみ届ける。
   */
  createTextNode(value: string): TsubameNode {
    return createTextShadowNode(value);
  },

  replaceText(textNode: TsubameNode, value: string): void {
    if (textNode.kind !== 'text') return;
    textNode.text = value;
    // 親 ElementNode の setText を更新する
    if (textNode.parent !== null) {
      refreshText(textNode.parent);
    }
  },

  isTextNode(node: TsubameNode): boolean {
    return node.kind === 'text';
  },

  setProperty(node: TsubameNode, name: string, value: unknown): void {
    if (node.kind !== 'element') return;
    const r = activeRenderer();

    if (name === 'style') {
      r.setStyle(node.id, (value ?? {}) as StylePatch);
      return;
    }

    const eventKind = EVENT_PROP[name];
    if (eventKind !== undefined) {
      node.events.get(name)?.();
      node.events.delete(name);
      if (typeof value === 'function') {
        node.events.set(
          name,
          r.addEventListener(node.id, eventKind, value as EventHandler),
        );
      }
      return;
    }
    // その他の prop（src / value 等）は MVP スコープ外。
  },

  insertNode(parent: TsubameNode, node: TsubameNode, anchor?: TsubameNode): void {
    if (parent.kind !== 'element') return;
    node.parent = parent;

    // shadow ツリーにスプライス
    if (anchor !== undefined) {
      const i = parent.children.indexOf(anchor);
      parent.children.splice(i < 0 ? parent.children.length : i, 0, node);
    } else {
      parent.children.push(node);
    }

    if (node.kind === 'text') {
      // TextNode は IRenderer ツリーに挿入しない。親の setText を更新する。
      refreshText(parent);
      return;
    }

    // ElementNode: IRenderer ツリーに実際に挿入する。
    // anchor が TextNode（IRenderer 非存在）の場合は次の実 ElementNode を使う。
    const r = activeRenderer();
    const realAnchor =
      anchor === undefined
        ? undefined
        : anchor.kind === 'element'
        ? anchor
        : nextElementSibling(parent, anchor);

    if (realAnchor !== undefined) {
      r.insertBefore(parent.id, node.id, realAnchor.id);
    } else {
      r.appendChild(parent.id, node.id);
    }
  },

  removeNode(parent: TsubameNode, node: TsubameNode): void {
    if (parent.kind !== 'element') return;
    const i = parent.children.indexOf(node);
    if (i >= 0) parent.children.splice(i, 1);
    node.parent = null;

    if (node.kind === 'text') {
      // TextNode は IRenderer ツリーになかったので removeChild は不要。
      refreshText(parent);
      return;
    }

    activeRenderer().removeChild(parent.id, node.id);
    disposeEvents(node);
  },

  getParentNode(node: TsubameNode): TsubameNode | undefined {
    return node.parent ?? undefined;
  },

  getFirstChild(node: TsubameNode): TsubameNode | undefined {
    return node.kind === 'element' ? node.children[0] : undefined;
  },

  getNextSibling(node: TsubameNode): TsubameNode | undefined {
    const parent = node.parent;
    if (parent === null) return undefined;
    const i = parent.children.indexOf(node);
    return i >= 0 ? parent.children[i + 1] : undefined;
  },
});

export {
  render,
  effect,
  memo,
  createComponent,
  createElement,
  createTextNode,
  insertNode,
  insert,
  spread,
  setProp,
  mergeProps,
};

export type { ElementNode };
