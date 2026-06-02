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
import type { RawHayate } from './hayate.js';
import { encodeStylePatch, unsetKindsOf } from './style-encoder.js';
import { ELEMENT_KIND, OP } from './opcodes.js';

export interface CanvasRendererOptions {
  requestFrame?: (cb: FrameRequestCallback) => number;
  cancelFrame?: (handle: number) => void;
}

const BUBBLING_EVENTS: ReadonlySet<EventKind> = new Set([
  'click',
  'input',
  'change',
  'keydown',
]);

type HandlerMap = Map<EventKind, Set<EventHandler>>;

export class CanvasRenderer implements IRenderer {
  private readonly raw: RawHayate;
  private readonly handlers = new Map<ElementId, HandlerMap>();
  private readonly parentOf = new Map<ElementId, ElementId>();
  private readonly childrenOf = new Map<ElementId, Set<ElementId>>();
  private nextId = 1;

  /** Per-frame mutation batch (ADR-0039). Flushed once per frame via `apply_mutations`. */
  private readonly ops: number[] = [];
  /** Flat style-packet buffer referenced by `OP_SET_STYLE` offsets. */
  private readonly styles: number[] = [];

  private readonly requestFrame: (cb: FrameRequestCallback) => number;
  private readonly cancelFrame: (handle: number) => void;
  private frameHandle: number | null = null;

  constructor(raw: RawHayate, options: CanvasRendererOptions = {}) {
    this.raw = raw;
    this.requestFrame =
      options.requestFrame ?? globalThis.requestAnimationFrame.bind(globalThis);
    this.cancelFrame =
      options.cancelFrame ?? globalThis.cancelAnimationFrame.bind(globalThis);
    this.frameHandle = this.requestFrame(this.frame);
  }

  stop(): void {
    if (this.frameHandle !== null) {
      this.cancelFrame(this.frameHandle);
      this.frameHandle = null;
    }
  }

  resize(width: number, height: number): void {
    this.raw.on_resize(width, height);
  }

  createElement(kind: ElementKind): ElementId {
    const id = this.nextId++;
    this.ops.push(OP.CREATE, id, ELEMENT_KIND[kind]);
    return asElementId(id);
  }

  setRoot(id: ElementId): void {
    this.ops.push(OP.SET_ROOT, id as number);
  }

  appendChild(parent: ElementId, child: ElementId): void {
    this.linkParent(parent, child);
    this.ops.push(OP.APPEND_CHILD, parent as number, child as number);
  }

  insertBefore(parent: ElementId, child: ElementId, before: ElementId): void {
    this.linkParent(parent, child);
    this.ops.push(
      OP.INSERT_BEFORE,
      parent as number,
      child as number,
      before as number,
    );
  }

  removeChild(_parent: ElementId, child: ElementId): void {
    this.ops.push(OP.REMOVE, child as number);
    this.pruneLocalSubtree(child);
  }

  setStyle(id: ElementId, style: StylePatch): void {
    // SET part → style-packet appended to the shared per-frame buffer.
    const offset = this.styles.length;
    encodeStylePatch(style, this.styles);
    const len = this.styles.length - offset;
    if (len > 0) {
      this.ops.push(OP.SET_STYLE, id as number, offset, len);
    }

    // UNSET part (ADR-0047) is out-of-band — `element_unset_style` is not an op
    // in `apply_mutations`. Flush first so a same-patch SET applies before the
    // reset, preserving order.
    const kinds = unsetKindsOf(style);
    if (kinds.length > 0) {
      this.flush();
      this.raw.element_unset_style(id as number, Uint32Array.from(kinds));
    }
  }

  setText(id: ElementId, text: string): void {
    // Text is a string op, kept out of the typed-array batch. Flush pending ops
    // so the element's OP_CREATE has been applied before we set its text.
    this.flush();
    this.raw.element_set_text(id as number, text);
  }

  setProperty(_id: ElementId, _name: string, _value: unknown): void {}

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
      if (set.size === 0) byKind.delete(event);
      if (byKind.size === 0) this.handlers.delete(id);
    };
  }

  /** Drain the per-frame batch into a single `apply_mutations` call. */
  private flush(): void {
    if (this.ops.length === 0) return;
    this.raw.apply_mutations(
      new Float64Array(this.ops),
      new Float32Array(this.styles),
    );
    this.ops.length = 0;
    this.styles.length = 0;
  }

  private readonly frame = (timestampMs: number): void => {
    this.flush();
    this.raw.render(timestampMs);
    this.dispatchEvents(this.raw.poll_events());
    this.frameHandle = this.requestFrame(this.frame);
  };

  /**
   * Decode Hayate's `poll_events()` array-of-arrays (ADR-0034) into protocol
   * events. Each entry is `[kindCode, ...fields]`; kind codes match
   * `encode_events` in `element_renderer.rs`. Events without a protocol
   * `EventKind` (resize / pointer-move / scroll / composition / active-*) are
   * ignored.
   */
  private dispatchEvents(events: unknown[]): void {
    for (const entry of events) {
      const ev = entry as Array<number | string>;
      switch (ev[0]) {
        case 0: // click [0, target, x, y]
          this.dispatchOne('click', asElementId(ev[1] as number));
          break;
        case 1: // focus [1, target]
          this.dispatchOne('focus', asElementId(ev[1] as number));
          break;
        case 2: // blur [2, target]
          this.dispatchOne('blur', asElementId(ev[1] as number));
          break;
        case 3: // text-input [3, target, text]
          this.dispatchOne('input', asElementId(ev[1] as number), {
            value: ev[2] as string,
          });
          break;
        case 10: // hover-enter [10, target]
          this.dispatchOne('hover-enter', asElementId(ev[1] as number));
          break;
        case 11: // hover-leave [11, target]
          this.dispatchOne('hover-leave', asElementId(ev[1] as number));
          break;
        case 12: // key-down [12, target, key, modifiers]
          this.dispatchOne('keydown', asElementId(ev[1] as number), {
            key: ev[2] as string,
          });
          break;
        default:
          break;
      }
    }
  }

  private dispatchOne(
    kind: EventKind,
    hit: ElementId,
    detail?: { value?: string; key?: string },
  ): void {
    const bubbles = BUBBLING_EVENTS.has(kind);
    let node: ElementId | undefined = hit;
    while (node !== undefined) {
      const set = this.handlers.get(node)?.get(kind);
      if (set !== undefined) {
        for (const handler of set) handler({ kind, target: node, ...detail });
      }
      if (!bubbles) break;
      node = this.parentOf.get(node);
    }
  }

  private linkParent(parent: ElementId, child: ElementId): void {
    const prevParent = this.parentOf.get(child);
    if (prevParent !== undefined) {
      this.childrenOf.get(prevParent)?.delete(child);
    }
    this.parentOf.set(child, parent);
    let set = this.childrenOf.get(parent);
    if (set === undefined) {
      set = new Set();
      this.childrenOf.set(parent, set);
    }
    set.add(child);
  }

  private pruneLocalSubtree(root: ElementId): void {
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
      this.handlers.delete(node);
    }
  }
}
