import type {
  ElementId,
  ElementKind,
  EventHandler,
  EventKind,
  IRenderer,
  InteractionEvent,
  StylePatch,
  Unsubscribe,
} from '@tsubame/renderer-protocol';
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

function elementIdFromDom(el: Element): ElementId | undefined {
  const raw = el.getAttribute('data-tsubame-id');
  if (raw === null) return undefined;
  const n = Number(raw);
  return Number.isFinite(n) ? asElementId(n) : undefined;
}

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
    this.node(parent).appendChild(this.node(child));
  }

  insertBefore(parent: ElementId, child: ElementId, before: ElementId): void {
    this.node(parent).insertBefore(this.node(child), this.node(before));
  }

  removeChild(_parent: ElementId, child: ElementId): void {
    const el = this.node(child);
    el.parentElement?.removeChild(el);
    this.forgetDomSubtree(el);
  }

  setStyle(id: ElementId, style: StylePatch): void {
    for (const key of Object.keys(style)) {
      if (style[key as keyof StylePatch] !== undefined) {
        warnZOrderDivergence(id, key);
      }
    }
    applyStylePatch(this.node(id), style);
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

  /** Drop `nodes` entries for `root` and registered DOM descendants. */
  private forgetDomSubtree(root: HTMLElement): void {
    const stack: Element[] = [root];
    while (stack.length > 0) {
      const el = stack.pop()!;
      for (const child of el.children) {
        stack.push(child);
      }
      const id = elementIdFromDom(el);
      if (id !== undefined) {
        this.nodes.delete(id);
      }
    }
  }
}
