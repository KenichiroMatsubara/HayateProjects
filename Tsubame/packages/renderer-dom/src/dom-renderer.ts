import type {
  ElementId,
  ElementKind,
  EventHandler,
  EventKind,
  IRenderer,
  InteractionEvent,
  PseudoStyleKey,
  StylePatch,
  Unsubscribe,
} from '@tsubame/renderer-protocol';
import { CATALOG_BY_KEY, formatDomCSSValue } from '@tsubame/hayate-css-catalog';
import { asElementId } from '@tsubame/renderer-protocol';
import { createDomElement } from './dom-elements.js';
import { applyStylePatch } from './style-mapping.js';
import { DOM_EVENT_NAME } from './event-mapping.js';
import { warnZOrderDivergence } from './z-order-divergence.js';

export interface DomRendererOptions {
  container?: HTMLElement;
  document?: Document;
}

function isTextInputLike(el: EventTarget | null): el is HTMLInputElement | HTMLTextAreaElement {
  return el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement;
}

function eventPayload(id: ElementId, eventKind: EventKind, event: Event): InteractionEvent {
  const target = event.target instanceof HTMLElement ? event.target : event.currentTarget;
  const payload: InteractionEvent = { kind: eventKind, target: id };

  if (isTextInputLike(target)) {
    payload.value = target.value;
  }
  if (event instanceof KeyboardEvent) {
    payload.key = event.key;
  }

  return payload;
}

export class DomRenderer implements IRenderer {
  private readonly doc: Document;
  private readonly container: HTMLElement;
  private readonly nodes = new Map<ElementId, HTMLElement>();
  private readonly parentOf = new Map<ElementId, ElementId>();
  private readonly childrenOf = new Map<ElementId, Set<ElementId>>();
  private readonly pseudoRuleKeys = new Map<string, number>();
  private readonly pseudoStyleEl: HTMLStyleElement;
  private nextId = 1;

  constructor(options: DomRendererOptions = {}) {
    this.doc = options.document ?? globalThis.document;
    this.container = options.container ?? this.doc.body;
    this.pseudoStyleEl = this.doc.createElement('style');
    this.pseudoStyleEl.setAttribute('data-tsubame-pseudo', '');
    this.doc.head.appendChild(this.pseudoStyleEl);
  }

  createElement(kind: ElementKind): ElementId {
    const id = asElementId(this.nextId++);
    const el = createDomElement(this.doc, kind);
    el.setAttribute('data-tsubame-id', String(id as number));
    this.nodes.set(id, el);
    return id;
  }

  setRoot(id: ElementId): void {
    const root = this.node(id);
    this.container.replaceChildren(root);
  }

  appendChild(parent: ElementId, child: ElementId): void {
    this.linkParent(parent, child);
    this.node(parent).appendChild(this.node(child));
  }

  insertBefore(parent: ElementId, child: ElementId, before: ElementId): void {
    this.linkParent(parent, child);
    this.node(parent).insertBefore(this.node(child), this.node(before));
  }

  removeChild(_parent: ElementId, child: ElementId): void {
    const el = this.node(child);
    el.parentElement?.removeChild(el);
    this.pruneSubtree(child);
  }

  setStyle(id: ElementId, style: StylePatch): void {
    for (const key of Object.keys(style)) {
      if (style[key as keyof StylePatch] !== undefined) {
        warnZOrderDivergence(id, key);
      }
    }
    applyStylePatch(this.node(id), style);
  }

  setPseudoStyle(id: ElementId, pseudo: PseudoStyleKey, style: StylePatch): void {
    const selector = `[data-tsubame-id="${id as number}"]${pseudo}`;
    const body = pseudoStyleDeclarations(style);
    if (body.length === 0) return;
    const sheet = this.pseudoStyleEl.sheet;
    if (sheet === null) return;
    const key = `${id as number}${pseudo}`;
    const cssText = `${selector}{${body}}`;
    const existing = this.pseudoRuleKeys.get(key);
    if (existing !== undefined) {
      const rule = sheet.cssRules.item(existing);
      if (rule !== null && 'style' in rule) {
        (rule as CSSStyleRule).style.cssText = body;
        return;
      }
      sheet.deleteRule(existing);
    }
    const index = sheet.insertRule(cssText, sheet.cssRules.length);
    this.pseudoRuleKeys.set(key, index);
  }

  setText(id: ElementId, text: string): void {
    this.node(id).textContent = text;
  }

  setProperty(id: ElementId, name: string, value: unknown): void {
    const target = this.node(id);

    switch (name) {
      case 'value':
        if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
          target.value = typeof value === 'string' ? value : value == null ? '' : String(value);
        }
        return;
      case 'placeholder':
        if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
          target.placeholder = typeof value === 'string' ? value : '';
        }
        return;
      case 'disabled':
        if (
          target instanceof HTMLInputElement ||
          target instanceof HTMLButtonElement ||
          target instanceof HTMLTextAreaElement
        ) {
          target.disabled = Boolean(value);
        }
        return;
      case 'src':
        if (target instanceof HTMLImageElement) {
          if (typeof value === 'string' && value.length > 0) target.src = value;
          else target.removeAttribute('src');
        }
        return;
      default:
        break;
    }

    if (value == null || value === false) {
      target.removeAttribute(name);
      return;
    }
    if (value === true) {
      target.setAttribute(name, '');
      return;
    }
    target.setAttribute(name, String(value));
  }

  addEventListener(
    id: ElementId,
    event: EventKind,
    handler: EventHandler,
  ): Unsubscribe {
    const target = this.node(id);
    const domEvent = DOM_EVENT_NAME[event];
    const listener = (nativeEvent: Event): void => {
      handler(eventPayload(id, event, nativeEvent));
    };
    target.addEventListener(domEvent, listener);
    return () => target.removeEventListener(domEvent, listener);
  }

  resize(_width: number, _height: number): void {}

  private node(id: ElementId): HTMLElement {
    const el = this.nodes.get(id);
    if (el === undefined) {
      throw new Error(`DomRenderer: unknown ElementId ${id as number}`);
    }
    return el;
  }

  private linkParent(parent: ElementId, child: ElementId): void {
    const prevParent = this.parentOf.get(child);
    if (prevParent !== undefined) {
      this.childrenOf.get(prevParent)?.delete(child);
    }
    this.parentOf.set(child, parent);
    let children = this.childrenOf.get(parent);
    if (children === undefined) {
      children = new Set();
      this.childrenOf.set(parent, children);
    }
    children.add(child);
  }

  private pruneSubtree(root: ElementId): void {
    for (const pseudo of [':hover', ':active', ':focus'] as const) {
      this.pseudoRuleKeys.delete(`${root as number}${pseudo}`);
    }
    const parent = this.parentOf.get(root);
    if (parent !== undefined) {
      this.childrenOf.get(parent)?.delete(root);
      this.parentOf.delete(root);
    }

    const stack: ElementId[] = [root];
    while (stack.length > 0) {
      const node = stack.pop()!;
      const children = this.childrenOf.get(node);
      if (children !== undefined) {
        for (const child of children) {
          this.parentOf.delete(child);
          stack.push(child);
        }
        this.childrenOf.delete(node);
      }
      this.nodes.delete(node);
    }
  }
}

function pseudoStyleDeclarations(patch: StylePatch): string {
  const parts: string[] = [];
  for (const key in patch) {
    const k = key as keyof StylePatch;
    const value = patch[k];
    if (value === undefined || value === null) continue;
    const entry = CATALOG_BY_KEY[k as string];
    if (entry === undefined) continue;
    parts.push(`${entry.cssName}:${formatDomCSSValue(entry, value)}`);
  }
  return parts.join(';');
}
