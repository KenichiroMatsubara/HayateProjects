import { createRenderer } from 'solid-js/universal';
import type {
  ElementKind,
  EventHandler,
  StylePatch,
} from '@tsubame/renderer-protocol';
import { splitHayateStyle } from '@tsubame/renderer-protocol';
import { activeRenderer } from './active-renderer.js';
import {
  createElementNode,
  isTextInTextCollapse,
  type TsubameNode,
} from './node.js';
import { EVENT_PROP } from './events.js';

function disposeEvents(node: TsubameNode): void {
  for (const unsub of node.events.values()) unsub();
  node.events.clear();
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
    return createElementNode(id, 'text', value);
  },

  replaceText(textNode: TsubameNode, value: string): void {
    if (textNode.elementKind !== 'text') return;
    textNode.text = value;
    const parent = textNode.parent;
    if (parent !== null && isTextInTextCollapse(parent, textNode)) {
      activeRenderer().setText(parent.id, value);
      return;
    }
    activeRenderer().setText(textNode.id, value);
  },

  isTextNode(node: TsubameNode): boolean {
    return node.elementKind === 'text';
  },

  setProperty(node: TsubameNode, name: string, value: unknown): void {
    if (node.elementKind === 'text') return;
    const r = activeRenderer();

    if (name === 'style') {
      const { base, pseudo } = splitHayateStyle(
        (value ?? {}) as Record<string, unknown>,
      );
      r.setStyle(node.id, base);
      for (const [key, block] of Object.entries(pseudo)) {
        if (block !== undefined) {
          r.setPseudoStyle(
            node.id,
            key as ':hover' | ':active' | ':focus',
            block,
          );
        }
      }
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

    if (name === 'children' || name === 'ref') return;
    r.setProperty(node.id, name, value);
  },

  insertNode(parent: TsubameNode, node: TsubameNode, anchor?: TsubameNode | null): void {
    node.parent = parent;
    insertIntoChildren(parent, node, anchor);

    const r = activeRenderer();

    if (isTextInTextCollapse(parent, node)) {
      r.setText(parent.id, node.text);
      r.removeChild(parent.id, node.id);
      return;
    }

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

    if (isTextInTextCollapse(parent, node)) {
      activeRenderer().setText(parent.id, '');
      activeRenderer().removeChild(parent.id, node.id);
      return;
    }

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
