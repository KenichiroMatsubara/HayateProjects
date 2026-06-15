import type {
  ElementId,
  ElementKind,
  EventHandler,
  EventKind,
  IRenderer,
  InteractionEvent,
  StylePatch,
  Unsubscribe,
  ViewportCondition,
} from '@tsubame/renderer-protocol';
import {
  asElementId,
  assertKnownElementProperty,
  coerceElementProperty,
  PSEUDO_STATE_PRIORITY,
  PSEUDO_STYLE_KEYS,
  type PseudoStyleKey,
} from '@tsubame/renderer-protocol';
import { createDomElement } from './dom-elements.js';
import { applyStylePatch } from './style-mapping.js';
import {
  declarationsFromStylePatch,
  declarationsToRuleBody,
} from './style-declarations.js';
import { DOM_EVENT_NAME } from './event-mapping.js';
import { warnZOrderDivergence } from './z-order-divergence.js';
import { resolveUserSelect } from './user-select.js';

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
  private readonly kinds = new Map<ElementId, ElementKind>();
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
    // ADR-0097 decision 5: the Selection Region default is `user-select: none`;
    // only a `selectable` subtree (and always text-input) opts into native
    // selection. Baseline it here so every element starts bounded.
    el.style.userSelect = resolveUserSelect(kind, undefined);
    this.nodes.set(id, el);
    this.kinds.set(id, kind);
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
    applyStylePatch(this.node(id), this.kindOf(id), style);
  }

  setPseudoStyle(id: ElementId, pseudo: PseudoStyleKey, style: StylePatch): void {
    const selector = `[data-tsubame-id="${id as number}"]${pseudo}`;
    const body = pseudoStyleDeclarations(this.kindOf(id), style);
    if (body.length === 0) return;
    const sheet = this.pseudoStyleEl.sheet;
    if (sheet === null) return;
    const key = `${id as number}${pseudo}`;
    const cssText = `${selector}{${body}}`;
    const priority = PSEUDO_STATE_PRIORITY[pseudo];
    const existing = this.pseudoRuleKeys.get(key);
    if (existing !== undefined) {
      const rule = sheet.cssRules.item(existing);
      if (rule !== null && 'style' in rule) {
        (rule as CSSStyleRule).style.cssText = body;
        return;
      }
      sheet.deleteRule(existing);
      this.bumpPseudoRuleIndices(existing, -1);
      this.pseudoRuleKeys.delete(key);
    }
    const index = insertionIndexForPseudoBand(sheet, priority);
    sheet.insertRule(cssText, index);
    this.bumpPseudoRuleIndices(index, 1);
    this.pseudoRuleKeys.set(key, index);
  }

  /** Viewport-conditional style override, output as `@media (...)` (ADR-0081). */
  setStyleVariant(id: ElementId, condition: ViewportCondition, style: StylePatch): void {
    const media = mediaQueryFor(condition);
    const selector = `[data-tsubame-id="${id as number}"]`;
    const body = pseudoStyleDeclarations(this.kindOf(id), style);
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
    const op = coerceElementProperty(name, value);

    switch (op.kind) {
      case 'text-content':
        if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
          target.value = op.text;
        }
        return;
      case 'placeholder':
        if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
          target.placeholder = op.text;
        }
        return;
      case 'disabled':
        if (
          target instanceof HTMLInputElement ||
          target instanceof HTMLButtonElement ||
          target instanceof HTMLTextAreaElement
        ) {
          target.disabled = op.disabled;
        }
        return;
      case 'src':
        if (target instanceof HTMLImageElement) {
          if (op.text.length > 0) target.src = op.text;
          else target.removeAttribute('src');
        }
        return;
      case 'selectable':
        // DOM Mode uses the browser's native selection; `selectable` only
        // bounds the Selection Region via `user-select` (ADR-0097 decision 5).
        // text-input stays selectable regardless of the boundary.
        if (target instanceof HTMLElement) {
          const value = resolveUserSelect(this.kindOf(id), op.selectable);
          target.style.userSelect = value;
          target.style.webkitUserSelect = value;
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

  /** Shift tracked rule indices when the pseudo stylesheet gains or loses a rule. */
  private bumpPseudoRuleIndices(from: number, delta: number): void {
    for (const [key, idx] of this.pseudoRuleKeys) {
      if (idx >= from) {
        this.pseudoRuleKeys.set(key, idx + delta);
      }
    }
  }

  private node(id: ElementId): HTMLElement {
    const el = this.nodes.get(id);
    if (el === undefined) {
      throw new Error(`DomRenderer: unknown ElementId ${id as number}`);
    }
    return el;
  }

  private kindOf(id: ElementId): ElementKind {
    const kind = this.kinds.get(id);
    if (kind === undefined) {
      throw new Error(`DomRenderer: unknown ElementId ${id as number}`);
    }
    return kind;
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
        for (const pseudo of PSEUDO_STYLE_KEYS) {
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
        this.kinds.delete(id);
      }
    }
  }
}

function pseudoPriorityFromSelector(selectorText: string): number {
  for (const pseudo of PSEUDO_STYLE_KEYS) {
    if (selectorText.endsWith(pseudo)) {
      return PSEUDO_STATE_PRIORITY[pseudo];
    }
  }
  return 0;
}

function insertionIndexForPseudoBand(sheet: CSSStyleSheet, priority: number): number {
  for (let i = 0; i < sheet.cssRules.length; i++) {
    const rule = sheet.cssRules[i] as CSSStyleRule;
    const rulePriority = pseudoPriorityFromSelector(rule.selectorText);
    if (rulePriority > priority) {
      return i;
    }
  }
  return sheet.cssRules.length;
}

function pseudoStyleDeclarations(kind: ElementKind, patch: StylePatch): string {
  return declarationsToRuleBody(
    declarationsFromStylePatch(kind, patch, { onUnknownKey: 'skip' }),
  );
}
