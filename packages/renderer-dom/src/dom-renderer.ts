import type {
  ElementId,
  ElementKind,
  EventHandler,
  EventKind,
  IRenderer,
  StylePatch,
  Unsubscribe,
} from '@tsubame/renderer-protocol';
import { asElementId } from '@tsubame/renderer-protocol';
import { createDomElement } from './element-mapping.js';
import { applyStylePatch } from './style-mapping.js';
import { DOM_EVENT_NAME } from './event-mapping.js';

export interface DomRendererOptions {
  /** ルート element をマウントするコンテナ。省略時は `document.body`。 */
  container?: HTMLElement;
  /** テスト等で差し替えるための Document。省略時は `globalThis.document`。 */
  document?: Document;
}

/**
 * Renderer Protocol の DOM 実装。Hayate（WASM）を一切使用しない純 JS CSR。
 *
 * ElementId は JS 側モノトニックカウンターで採番し、Map で DOM ノードと
 * 対応付ける（ADR-0005）。JS→WASM 境界は存在しない。
 */
export class DomRenderer implements IRenderer {
  private readonly doc: Document;
  private readonly container: HTMLElement;
  private readonly nodes = new Map<ElementId, HTMLElement>();
  private nextId = 1;

  constructor(options: DomRendererOptions = {}) {
    this.doc = options.document ?? globalThis.document;
    this.container = options.container ?? this.doc.body;
  }

  createElement(kind: ElementKind): ElementId {
    const id = asElementId(this.nextId++);
    this.nodes.set(id, createDomElement(this.doc, kind));
    return id;
  }

  setRoot(id: ElementId): void {
    const root = this.node(id);
    this.container.replaceChildren(root);
  }

  appendChild(parent: ElementId, child: ElementId): void {
    this.node(parent).appendChild(this.node(child));
  }

  insertBefore(parent: ElementId, child: ElementId, before: ElementId): void {
    this.node(parent).insertBefore(this.node(child), this.node(before));
  }

  removeChild(parent: ElementId, child: ElementId): void {
    this.node(parent).removeChild(this.node(child));
  }

  setStyle(id: ElementId, style: StylePatch): void {
    applyStylePatch(this.node(id), style);
  }

  setText(id: ElementId, text: string): void {
    this.node(id).textContent = text;
  }

  addEventListener(
    id: ElementId,
    event: EventKind,
    handler: EventHandler,
  ): Unsubscribe {
    const target = this.node(id);
    const domEvent = DOM_EVENT_NAME[event];
    const listener = (): void => handler({ kind: event, target: id });
    target.addEventListener(domEvent, listener);
    return () => target.removeEventListener(domEvent, listener);
  }

  /** 内部用: ElementId から DOM ノードを引く。未登録なら例外。 */
  private node(id: ElementId): HTMLElement {
    const el = this.nodes.get(id);
    if (el === undefined) {
      throw new Error(`DomRenderer: unknown ElementId ${id as number}`);
    }
    return el;
  }
}
