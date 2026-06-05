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
import { HayateMutationPacket } from './hayate-mutation-packet.js';
import { parseEvent } from './protocol.js';

export interface CanvasRendererOptions {
  requestFrame?: (cb: FrameRequestCallback) => number;
  cancelFrame?: (handle: number) => void;
  canvas?: HTMLCanvasElement;
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

  private readonly packet: HayateMutationPacket;

  private readonly canvas: HTMLCanvasElement | null;
  private readonly requestFrame: (cb: FrameRequestCallback) => number;
  private readonly cancelFrame: (handle: number) => void;
  private frameHandle: number | null = null;

  constructor(raw: RawHayate, options: CanvasRendererOptions = {}) {
    this.raw = raw;
    this.packet = new HayateMutationPacket(raw);
    this.canvas = options.canvas ?? null;
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
    if (this.canvas !== null) {
      this.canvas.width = width;
      this.canvas.height = height;
    }
    this.raw.on_resize(width, height);
  }

  createElement(kind: ElementKind): ElementId {
    const id = asElementId(this.nextId++);
    this.packet.enqueueCreateElement(id, kind);
    return id;
  }

  setRoot(id: ElementId): void {
    this.packet.enqueueSetRoot(id);
  }

  appendChild(parent: ElementId, child: ElementId): void {
    this.linkParent(parent, child);
    this.packet.enqueueAppendChild(parent, child);
  }

  insertBefore(parent: ElementId, child: ElementId, before: ElementId): void {
    this.linkParent(parent, child);
    this.packet.enqueueInsertBefore(parent, child, before);
  }

  removeChild(_parent: ElementId, child: ElementId): void {
    this.packet.enqueueRemove(child);
    this.pruneLocalSubtree(child);
  }

  setStyle(id: ElementId, style: StylePatch): void {
    this.packet.enqueueSetStyle(id, style);
  }

  setText(id: ElementId, text: string): void {
    this.packet.enqueueSetText(id, text);
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

  /** Drain the ordered mutation packet into the Hayate WASM boundary. */
  private flush(): void {
    this.packet.flush();
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
      const ev = parseEvent(entry as unknown[]);
      switch (ev.kind) {
        case 'click':
          this.dispatchOne('click', asElementId(ev.targetId));
          break;
        case 'focus':
          this.dispatchOne('focus', asElementId(ev.targetId));
          break;
        case 'blur':
          this.dispatchOne('blur', asElementId(ev.targetId));
          break;
        case 'text_input':
          this.dispatchOne('input', asElementId(ev.targetId), { value: ev.text });
          break;
        case 'hover_enter':
          this.dispatchOne('hover-enter', asElementId(ev.targetId));
          break;
        case 'hover_leave':
          this.dispatchOne('hover-leave', asElementId(ev.targetId));
          break;
        case 'key_down':
          this.dispatchOne('keydown', asElementId(ev.targetId), { key: ev.key });
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
