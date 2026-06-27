import { createRenderer } from 'solid-js/universal';
import type { ElementKind } from '@tsubame/renderer-protocol';
import { applyElementProp } from '@tsubame/renderer-protocol';
import { activeRenderer } from './active-renderer.js';
import { createElementNode, type TsubameNode } from './node.js';

function disposeEvents(node: TsubameNode): void {
  for (const unsub of node.listeners.values()) unsub();
  node.listeners.clear();
  for (const child of node.children) disposeEvents(child);
}

function insertIntoChildren(
  parent: TsubameNode,
  node: TsubameNode,
  anchor?: TsubameNode | null,
): void {
  if (anchor != null) {
    const i = parent.children.indexOf(anchor);
    parent.children.splice(i < 0 ? parent.children.length : i, 0, node);
  } else {
    parent.children.push(node);
  }
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
    const kind = tag as ElementKind;
    const id = activeRenderer().createElement(kind);
    return createElementNode(id, kind);
  },

  createTextNode(value: string): TsubameNode {
    const r = activeRenderer();
    const id = r.createElement('text');
    r.setText(id, value);
    return createElementNode(id, 'text');
  },

  replaceText(textNode: TsubameNode, value: string): void {
    if (textNode.kind !== 'text') return;
    activeRenderer().setText(textNode.id, value);
  },

  isTextNode(node: TsubameNode): boolean {
    return node.kind === 'text';
  },

  setProperty(node: TsubameNode, name: string, value: unknown): void {
    // style チャンネル・event 語彙・閉じた要素プロパティの dispatch は
    // `tsubame-react` と共通の `applyElementProp` seam に委譲する（ADR-0010）。
    applyElementProp(activeRenderer(), node, name, value);
  },

  insertNode(parent: TsubameNode, node: TsubameNode, anchor?: TsubameNode | null): void {
    node.parent = parent;
    insertIntoChildren(parent, node, anchor);

    const r = activeRenderer();
    if (anchor == null) {
      r.appendChild(parent.id, node.id);
      return;
    }
    r.insertBefore(parent.id, node.id, anchor.id);
  },

  removeNode(parent: TsubameNode, node: TsubameNode): void {
    const i = parent.children.indexOf(node);
    if (i >= 0) parent.children.splice(i, 1);
    node.parent = null;

    activeRenderer().removeChild(parent.id, node.id);
    disposeEvents(node);
  },

  getParentNode(node: TsubameNode): TsubameNode | undefined {
    return node.parent ?? undefined;
  },

  getFirstChild(node: TsubameNode): TsubameNode | undefined {
    return node.children[0];
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

export type { TsubameNode as ElementNode };
