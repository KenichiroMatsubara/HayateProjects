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
  dispatchElementPropertyOp,
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
  /// Listeners registered per element, kept so they can be re-bound when the
  /// underlying DOM node is replaced (the `<input>`↔`<textarea>` swap, #362).
  private readonly domListeners = new Map<
    ElementId,
    Set<{ domEvent: string; listener: (e: Event) => void }>
  >();
  private readonly pseudoRules = new Map<string, { index: number; priority: number }>();
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
    // ADR-0108: selectability defaults to the element-kind UA default (view /
    // text / scroll-view selectable, button / image not; text-input always).
    // Baseline it here so every element starts at its kind default before any
    // explicit `user-select` arrives.
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
    const existing = this.pseudoRules.get(key);
    if (existing !== undefined) {
      const rule = sheet.cssRules[existing.index];
      if (rule !== undefined && 'style' in rule) {
        (rule as CSSStyleRule).style.cssText = body;
        return;
      }
      sheet.deleteRule(existing.index);
      this.bumpPseudoRuleIndices(existing.index, -1);
      this.pseudoRules.delete(key);
    }
    const index = this.insertionIndexForPseudoBand(priority);
    sheet.insertRule(cssText, index);
    this.bumpPseudoRuleIndices(index, 1);
    this.pseudoRules.set(key, { index, priority });
  }

  /** Viewport-conditional style override, output as `@media (...)` (ADR-0081). */
  setStyleVariant(id: ElementId, condition: ViewportCondition, style: StylePatch): void {
    const media = mediaQueryFor(condition);
    const selector = `[data-tsubame-id="${id as number}"]`;
    const body = variantStyleDeclarations(this.kindOf(id), style);
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

    // Shared spec-generated dispatch (ADR-0008): the DOM adapter fills only the
    // effect handlers — the op-kind match lives once in the protocol.
    dispatchElementPropertyOp<void>(op, {
      'text-content': ({ text }) => {
        if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
          target.value = text;
        }
      },
      placeholder: ({ text }) => {
        if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement) {
          target.placeholder = text;
        }
      },
      disabled: ({ disabled }) => {
        if (
          target instanceof HTMLInputElement ||
          target instanceof HTMLButtonElement ||
          target instanceof HTMLTextAreaElement
        ) {
          target.disabled = disabled;
        }
      },
      src: ({ text }) => {
        if (target instanceof HTMLImageElement) {
          if (text.length > 0) target.src = text;
          else target.removeAttribute('src');
        }
      },
      'user-select': ({ value }) => {
        // DOM Mode uses the browser's native selection; `user-select` resolves
        // through the element-kind default to a CSS `user-select` value
        // (ADR-0108). text-input stays selectable regardless of the value.
        if (target instanceof HTMLElement) {
          const userSelect = resolveUserSelect(this.kindOf(id), value);
          target.style.userSelect = userSelect;
          target.style.webkitUserSelect = userSelect;
        }
      },
      multiline: ({ multiline }) => this.setMultiline(id, multiline),
    });
  }

  /**
   * Swap a text-input's DOM node between `<input>` and `<textarea>` so the
   * browser's native Enter behaviour matches the `multiline` property (#362):
   * a textarea inserts a newline at the caret, an input submits. The live
   * value, placeholder, disabled flag, resolved inline styles, and registered
   * event listeners all carry across the swap.
   */
  private setMultiline(id: ElementId, multiline: boolean): void {
    if (this.kindOf(id) !== 'text-input') return;
    const oldEl = this.node(id);
    const isTextarea = oldEl instanceof HTMLTextAreaElement;
    if (isTextarea === multiline) return; // already the right element

    const newEl = createDomElement(this.doc, 'text-input', multiline);
    newEl.setAttribute('data-tsubame-id', String(id as unknown as number));
    // Carry the resolved inline styles (baseline + user + variant + userSelect).
    newEl.style.cssText = oldEl.style.cssText;
    // Carry the editable state across the input/textarea boundary.
    if (
      (oldEl instanceof HTMLInputElement || oldEl instanceof HTMLTextAreaElement) &&
      (newEl instanceof HTMLInputElement || newEl instanceof HTMLTextAreaElement)
    ) {
      newEl.value = oldEl.value;
      newEl.placeholder = oldEl.placeholder;
      newEl.disabled = oldEl.disabled;
    }
    // Re-bind every registered listener to the new node.
    const listeners = this.domListeners.get(id);
    if (listeners !== undefined) {
      for (const { domEvent, listener } of listeners) {
        oldEl.removeEventListener(domEvent, listener);
        newEl.addEventListener(domEvent, listener);
      }
    }
    oldEl.replaceWith(newEl);
    this.nodes.set(id, newEl);
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
    // Track the binding so it survives a node swap (`<input>`↔`<textarea>`, #362).
    const entry = { domEvent, listener };
    let set = this.domListeners.get(id);
    if (set === undefined) {
      set = new Set();
      this.domListeners.set(id, set);
    }
    set.add(entry);
    return () => {
      // Detach from whichever node currently backs the element (it may have been
      // swapped since registration).
      this.nodes.get(id)?.removeEventListener(domEvent, listener);
      this.domListeners.get(id)?.delete(entry);
    };
  }

  resize(_width: number, _height: number): void {}

  /** Shift tracked rule indices when the pseudo stylesheet gains or loses a rule. */
  private bumpPseudoRuleIndices(from: number, delta: number): void {
    for (const entry of this.pseudoRules.values()) {
      if (entry.index >= from) {
        entry.index += delta;
      }
    }
  }

  /**
   * Index at which a rule of the given band priority keeps the pseudo
   * stylesheet sorted ascending (focus < hover < active; last wins). Driven
   * entirely by the spec-generated `PSEUDO_STATE_PRIORITY` recorded per rule —
   * the sheet stays band-sorted, so the count of rules in lower-or-equal bands
   * is the first slot in the next band. No selector string is inspected.
   */
  private insertionIndexForPseudoBand(priority: number): number {
    let index = 0;
    for (const entry of this.pseudoRules.values()) {
      if (entry.priority <= priority) index += 1;
    }
    return index;
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
          this.pseudoRules.delete(`${id as number}${pseudo}`);
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
        this.domListeners.delete(id);
      }
    }
  }
}

function pseudoStyleDeclarations(kind: ElementKind, patch: StylePatch): string {
  return declarationsToRuleBody(
    declarationsFromStylePatch(kind, patch, { onUnknownKey: 'skip' }),
  );
}

/**
 * Viewport variant（@media）ルール本体。ベーススタイルはインライン（`el.style`）
 * に載るため、`!important` を付けないと variant がベースを上書きできない。これが
 * 無いと padding / flexDirection / display 等の狭幅オーバーライドが一切効かず、
 * DOM Mode でレスポンシブ（ADR-0081）が成立しない。
 */
function variantStyleDeclarations(kind: ElementKind, patch: StylePatch): string {
  return declarationsToRuleBody(
    declarationsFromStylePatch(kind, patch, { onUnknownKey: 'skip' }),
    { important: true },
  );
}
