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
import type { HayateWasm } from './hayate.js';
import {
  OP,
  KIND_CODE,
  EVENT_KIND_BY_CODE,
  EVENT_RECORD_SLOTS,
} from './opcodes.js';
import { encodeStylePatch } from './style-packet.js';

/** ops バッファ（Float64Array）の固定長。フレームごとに書き込み位置をリセットする。 */
export const MAX_OPS = 1 << 16;
/** styles バッファ（Float32Array）の固定長。 */
export const MAX_STYLE_FLOATS = 1 << 16;

/**
 * 祖先の handler までバブリングする EventKind。
 *
 * DOM Renderer はネイティブイベントのバブリングに依存して、子要素上の click を
 * 親（例: button > text）の handler へ届ける。Canvas Renderer も親子関係を保持して
 * 同じ意味論を再現し、両 Renderer の挙動を一致させる。focus/blur/hover は DOM でも
 * バブリングしないため対象外。
 */
const BUBBLING_EVENTS: ReadonlySet<EventKind> = new Set(['click']);

export interface CanvasRendererOptions {
  /** RAF スケジューラの差し替え（テスト用）。省略時は requestAnimationFrame。 */
  requestFrame?: (cb: FrameRequestCallback) => number;
  cancelFrame?: (handle: number) => void;
}

type HandlerMap = Map<EventKind, Set<EventHandler>>;

/**
 * Renderer Protocol の Canvas 実装。フレーム分の mutation を JS 側で積み、
 * `apply_mutations(ops, styles)` で 1 回/frame に集約して Hayate WASM に渡す
 * （JS→WASM 境界コストを O(1)/frame に削減）。
 *
 * - ElementId は JS 側モノトニックカウンターで採番（ADR-0005）
 * - createElement も OP_CREATE としてバッチに乗せる
 * - RAF ループは本クラスが所有し、コンストラクタで開始する
 * - RAF ループ内で poll_events() を呼び、登録済み handler を invoke する
 */
export class CanvasRenderer implements IRenderer {
  private readonly hayate: HayateWasm;
  private readonly ops = new Float64Array(MAX_OPS);
  private readonly styles = new Float32Array(MAX_STYLE_FLOATS);
  private opsCursor = 0;
  private styleCursor = 0;
  private readonly pendingText: Array<{ id: number; text: string }> = [];
  private readonly handlers = new Map<ElementId, HandlerMap>();
  /** child → parent。イベントのバブリングに用いる（ツリー操作 op から構築）。 */
  private readonly parentOf = new Map<ElementId, ElementId>();
  private nextId = 1;

  private readonly requestFrame: (cb: FrameRequestCallback) => number;
  private readonly cancelFrame: (handle: number) => void;
  private frameHandle: number | null = null;

  constructor(hayate: HayateWasm, options: CanvasRendererOptions = {}) {
    this.hayate = hayate;
    this.requestFrame =
      options.requestFrame ?? globalThis.requestAnimationFrame.bind(globalThis);
    this.cancelFrame =
      options.cancelFrame ?? globalThis.cancelAnimationFrame.bind(globalThis);
    this.frameHandle = this.requestFrame(this.frame);
  }

  /** RAF ループを停止する。 */
  stop(): void {
    if (this.frameHandle !== null) {
      this.cancelFrame(this.frameHandle);
      this.frameHandle = null;
    }
  }

  /** レンダリングサーフェスのサイズを更新する（IRenderer.resize 実装）。 */
  resize(width: number, height: number): void {
    this.hayate.resize?.(width, height);
  }

  createElement(kind: ElementKind): ElementId {
    const id = this.nextId++;
    this.pushOp(OP.CREATE, id, KIND_CODE[kind]);
    return asElementId(id);
  }

  setRoot(id: ElementId): void {
    this.pushOp(OP.SET_ROOT, id as number);
  }

  appendChild(parent: ElementId, child: ElementId): void {
    this.parentOf.set(child, parent);
    this.pushOp(OP.APPEND_CHILD, parent as number, child as number);
  }

  insertBefore(parent: ElementId, child: ElementId, before: ElementId): void {
    this.parentOf.set(child, parent);
    this.pushOp(
      OP.INSERT_BEFORE,
      parent as number,
      child as number,
      before as number,
    );
  }

  removeChild(_parent: ElementId, child: ElementId): void {
    this.parentOf.delete(child);
    this.pushOp(OP.REMOVE, child as number);
  }

  setStyle(id: ElementId, style: StylePatch): void {
    const offset = this.styleCursor;
    const len = encodeStylePatch(style, this.styles, offset);
    if (len === 0) return;
    this.styleCursor += len;
    if (this.styleCursor > MAX_STYLE_FLOATS) {
      throw new Error(
        `CanvasRenderer: styles バッファ超過（MAX_STYLE_FLOATS=${MAX_STYLE_FLOATS}）`,
      );
    }
    this.pushOp(OP.SET_STYLE, id as number, offset, len);
  }

  setText(id: ElementId, text: string): void {
    // 文字列 op はバッチ外（ADR-0003）。ただし OP_CREATE のフラッシュ後に
    // 適用されるよう、フレーム末で apply_mutations 後に実行する。
    this.pendingText.push({ id: id as number, text });
  }

  addEventListener(
    id: ElementId,
    event: EventKind,
    handler: EventHandler,
  ): Unsubscribe {
    let byKind = this.handlers.get(id);
    if (byKind === undefined) {
      byKind = new Map();
      this.handlers.set(id, byKind);
    }
    let set = byKind.get(event);
    if (set === undefined) {
      set = new Set();
      byKind.set(event, set);
    }
    set.add(handler);
    return () => {
      set.delete(handler);
    };
  }

  // --- フレームループ ---

  private readonly frame = (): void => {
    if (this.opsCursor > 0) {
      this.hayate.apply_mutations(
        this.ops.subarray(0, this.opsCursor),
        this.styles.subarray(0, this.styleCursor),
      );
      this.opsCursor = 0;
      this.styleCursor = 0;
    }

    if (this.pendingText.length > 0) {
      for (const { id, text } of this.pendingText) {
        this.hayate.element_set_text(id, text);
      }
      this.pendingText.length = 0;
    }

    this.dispatchEvents();
    this.frameHandle = this.requestFrame(this.frame);
  };

  private dispatchEvents(): void {
    // ADR-0034: poll_events() は Array<Array<any>> を返す。
    // 各サブ配列は [kind: number, target?: number, ...rest] の形式。
    const events = this.hayate.poll_events();
    for (const sub of events) {
      const kindCode = sub[0] as number;
      const kind = EVENT_KIND_BY_CODE[kindCode];
      if (kind === undefined) continue;
      // target フィールドがある種別（click, focus, blur, hover-enter/leave 等）
      const targetRaw = sub[1];
      if (typeof targetRaw !== 'number') continue;
      this.dispatchOne(kind, asElementId(targetRaw));
    }
  }

  /**
   * ヒットした element から、必要なら祖先へバブリングしつつ handler を invoke する。
   * DOM Renderer 同様、`target` は handler が登録された element とする。
   */
  private dispatchOne(kind: EventKind, hit: ElementId): void {
    const bubbles = BUBBLING_EVENTS.has(kind);
    let node: ElementId | undefined = hit;
    while (node !== undefined) {
      const set = this.handlers.get(node)?.get(kind);
      if (set !== undefined) {
        for (const handler of set) handler({ kind, target: node });
      }
      if (!bubbles) break;
      node = this.parentOf.get(node);
    }
  }

  private pushOp(...slots: number[]): void {
    if (this.opsCursor + slots.length > MAX_OPS) {
      throw new Error(
        `CanvasRenderer: ops バッファ超過（MAX_OPS=${MAX_OPS}）。1 フレームの mutation 数を見直してください。`,
      );
    }
    for (const slot of slots) this.ops[this.opsCursor++] = slot;
  }
}
