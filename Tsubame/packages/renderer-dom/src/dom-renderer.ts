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
  ViewportCondition,
} from '@tsubame/renderer-protocol';
import { CATALOG_BY_KEY, formatDomCSSValue } from '@tsubame/hayate-css-catalog';
import { asElementId, assertKnownElementProperty } from '@tsubame/renderer-protocol';
import { createDomElement } from './dom-elements.js';
import { applyStylePatch } from './style-mapping.js';
import { shouldApplyTextLocalPatch } from './text-style-semantics.js';
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

/** ADR-0081: build a `@media` query string from a property-level viewport condition. */
function mediaQueryFor(condition: ViewportCondition): string {
  const parts: string[] = [];
  if (condition.minWidth !== undefined) parts.push(`(min-width: ${condition.minWidth}px)`);
  if (condition.maxWidth !== undefined) parts.push(`(max-width: ${condition.maxWidth}px)`);
  if (condition.minHeight !== undefined) parts.push(`(min-height: ${condition.minHeight}px)`);
  if (condition.maxHeight !== undefined) parts.push(`(max-height: ${condition.maxHeight}px)`);
  if (parts.length === 0) {
    throw new Error('DomRenderer: setStyleVariant requires at least one viewport condition axis');
  }
  return parts.join(' and ');
}

export class DomRenderer implements IRenderer {
  private readonly doc: Document;
  private readonly container: HTMLElement;
  private readonly nodes = new Map<ElementId, HTMLElement>();
  private readonly pseudoRuleKeys = new Map<string, number>();
  private readonly pseudoStyleEl: HTMLStyleElement;
  private readonly variantRuleKeys = new Map<string, number>();
  private readonly variantMediaByElement = new Map<ElementId, Set<string>>();
  private readonly variantStyleEl: HTMLStyleElement;
  private nextId = 1;

  constructor(options: DomRendererOptions = {}) {
    this.doc = options.document ?? globalThis.document;
    this.container = options.container ?? this.doc.body;
    this.pseudoStyleEl = this.doc.createElement('style');
    this.pseudoStyleEl.setAttribute('data-tsubame-pseudo', '');
    this.doc.head.appendChild(this.pseudoStyleEl);
    this.variantStyleEl = this.doc.createElement('style');
    this.variantStyleEl.setAttribute('data-tsubame-variant', '');
    this.doc.head.appendChild(this.variantStyleEl);
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
    // Canvas 経路ではルートがサーフェスサイズで layout されるのに合わせ、
    // DOM 経路でもルートを container いっぱいに広げる。これがないと
    // 子の height:100% が解決できず scroll-view がスクロールしない。
    root.style.width = '100%';
    root.style.height = '100%';
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

  setPseudoStyle(id: ElementId, pseudo: PseudoStyleKey, style: StylePatch): void {
    const selector = `[data-tsubame-id="${id as number}"]${pseudo}`;
    const body = pseudoStyleDeclarations(this.node(id), style);
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

  /** Viewport-conditional style override, output as `@media (...)` (ADR-0081). */
  setStyleVariant(id: ElementId, condition: ViewportCondition, style: StylePatch): void {
    const media = mediaQueryFor(condition);
    const selector = `[data-tsubame-id="${id as number}"]`;
    const body = pseudoStyleDeclarations(this.node(id), style);
    if (body.length === 0) return;
    const sheet = this.variantStyleEl.sheet;
    if (sheet === null) return;
    const key = `${id as number}|${media}`;
    const cssText = `@media ${media}{${selector}{${body}}}`;
    const existing = this.variantRuleKeys.get(key);
    if (existing !== undefined) {
      const rule = sheet.cssRules[existing];
      if (rule !== undefined && 'cssRules' in rule) {
        const inner = (rule as CSSMediaRule).cssRules[0];
        if (inner !== undefined && 'style' in inner) {
          (inner as CSSStyleRule).style.cssText = body;
          return;
        }
      }
      sheet.deleteRule(existing);
    }
    const index = sheet.insertRule(cssText, sheet.cssRules.length);
    this.variantRuleKeys.set(key, index);
    let mediaSet = this.variantMediaByElement.get(id);
    if (mediaSet === undefined) {
      mediaSet = new Set();
      this.variantMediaByElement.set(id, mediaSet);
    }
    mediaSet.add(media);
  }

  setText(id: ElementId, text: string): void {
    this.node(id).textContent = text;
  }

  setProperty(id: ElementId, name: string, value: unknown): void {
    assertKnownElementProperty(name);
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
    }
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

  /** Drop `nodes` entries and pseudo rules for `root` and DOM descendants. */
  private forgetDomSubtree(root: HTMLElement): void {
    const stack: Element[] = [root];
    while (stack.length > 0) {
      const el = stack.pop()!;
      for (const child of el.children) {
        stack.push(child);
      }
      const id = elementIdFromDom(el);
      if (id !== undefined) {
        for (const pseudo of [':hover', ':active', ':focus'] as const) {
          this.pseudoRuleKeys.delete(`${id as number}${pseudo}`);
        }
        const mediaSet = this.variantMediaByElement.get(id);
        if (mediaSet !== undefined) {
          for (const media of mediaSet) {
            this.variantRuleKeys.delete(`${id as number}|${media}`);
          }
          this.variantMediaByElement.delete(id);
        }
        this.nodes.delete(id);
      }
    }
  }
}

function pseudoStyleDeclarations(el: HTMLElement, patch: StylePatch): string {
  const parts: string[] = [];
  for (const key in patch) {
    const k = key as keyof StylePatch;
    const value = patch[k];
    if (value === undefined || value === null) continue;
    if (!shouldApplyTextLocalPatch(el, k as string)) continue;
    const entry = CATALOG_BY_KEY[k as string];
    if (entry === undefined) continue;
    parts.push(`${entry.cssProperty}:${formatDomCSSValue(entry, value)}`);
  }
  return parts.join(';');
}
