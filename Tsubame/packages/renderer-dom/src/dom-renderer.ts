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

/** プロパティ単位のビューポート条件から `@media` クエリ文字列を組み立てる（ADR-0081）。 */
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
  /// 要素ごとに登録したリスナ。下層 DOM ノードの差し替え
  /// （`<input>`↔`<textarea>` 入れ替え）時に再バインドできるよう保持する。
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
    // 選択可否は要素種別の UA デフォルトに従う（view / text / scroll-view は
    // 選択可、button / image は不可、text-input は常に可）（ADR-0108）。明示的な
    // `user-select` が来る前に種別デフォルトで初期化しておく。
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

  /** ビューポート条件付きスタイル上書き。`@media (...)` として出力する（ADR-0081）。 */
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

    // spec 生成の共有ディスパッチ（ADR-0008）。DOM アダプタは効果ハンドラのみを
    // 埋め、op 種別の分岐はプロトコル側に一度だけ存在する。
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
        // DOM Mode はブラウザのネイティブ選択を使う。`user-select` は要素種別
        // デフォルトを介して CSS の `user-select` 値に解決される（ADR-0108）。
        // text-input は値に関わらず選択可能のまま。
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
   * text-input の DOM ノードを `<input>` と `<textarea>` の間で入れ替え、
   * ブラウザのネイティブ Enter 挙動を `multiline` プロパティに合わせる。
   * textarea はキャレット位置に改行を挿入し、input は送信する。値・placeholder・
   * disabled・解決済みインラインスタイル・登録済みリスナはすべて入れ替えを跨いで引き継ぐ。
   */
  private setMultiline(id: ElementId, multiline: boolean): void {
    if (this.kindOf(id) !== 'text-input') return;
    const oldEl = this.node(id);
    const isTextarea = oldEl instanceof HTMLTextAreaElement;
    if (isTextarea === multiline) return; // 既に正しい要素

    const newEl = createDomElement(this.doc, 'text-input', multiline);
    newEl.setAttribute('data-tsubame-id', String(id as unknown as number));
    // 解決済みインラインスタイル（baseline + user + variant + userSelect）を引き継ぐ。
    newEl.style.cssText = oldEl.style.cssText;
    // 編集状態を input/textarea の境界を跨いで引き継ぐ。
    if (
      (oldEl instanceof HTMLInputElement || oldEl instanceof HTMLTextAreaElement) &&
      (newEl instanceof HTMLInputElement || newEl instanceof HTMLTextAreaElement)
    ) {
      newEl.value = oldEl.value;
      newEl.placeholder = oldEl.placeholder;
      newEl.disabled = oldEl.disabled;
    }
    // 登録済みリスナをすべて新ノードへ再バインドする。
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
    // ノード入れ替え（`<input>`↔`<textarea>`）を跨いで残るようバインドを記録する。
    const entry = { domEvent, listener };
    let set = this.domListeners.get(id);
    if (set === undefined) {
      set = new Set();
      this.domListeners.set(id, set);
    }
    set.add(entry);
    return () => {
      // 現在その要素を支えているノードから解除する（登録後に入れ替わっている可能性がある）。
      this.nodes.get(id)?.removeEventListener(domEvent, listener);
      this.domListeners.get(id)?.delete(entry);
    };
  }

  resize(_width: number, _height: number): void {}

  /** 擬似スタイルシートのルール増減に合わせて、追跡中のルールインデックスをずらす。 */
  private bumpPseudoRuleIndices(from: number, delta: number): void {
    for (const entry of this.pseudoRules.values()) {
      if (entry.index >= from) {
        entry.index += delta;
      }
    }
  }

  /**
   * 指定バンド優先度のルールを挿入しても擬似スタイルシートが昇順
   * （focus < hover < active、後勝ち）を保つインデックス。ルール毎に記録した
   * spec 生成の `PSEUDO_STATE_PRIORITY` のみで決まる。シートは常にバンド整列
   * されているため、優先度が同等以下のルール数が次バンドの先頭スロットになる。
   * セレクタ文字列は一切参照しない。
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

  /** `root` と DOM 子孫の `nodes` エントリと擬似ルールを破棄する。 */
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
