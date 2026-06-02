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
import type { HayateEvent, HayateWasm } from './hayate.js';
import { stylePatchToMutation } from './hayate.js';

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
  private readonly hayate: HayateWasm;
  private readonly handlers = new Map<ElementId, HandlerMap>();
  private readonly parentOf = new Map<ElementId, ElementId>();
  private readonly childrenOf = new Map<ElementId, Set<ElementId>>();
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

  stop(): void {
    if (this.frameHandle !== null) {
      this.cancelFrame(this.frameHandle);
      this.frameHandle = null;
    }
  }

  resize(width: number, height: number): void {
    this.hayate.on_resize(width, height);
  }

  createElement(kind: ElementKind): ElementId {
    const id = this.nextId++;
    this.hayate.element_create(id, kind);
    return asElementId(id);
  }

  setRoot(id: ElementId): void {
    this.hayate.set_root(id as number);
  }

  appendChild(parent: ElementId, child: ElementId): void {
    this.linkParent(parent, child);
    this.hayate.element_append_child(parent as number, child as number);
  }

  insertBefore(parent: ElementId, child: ElementId, before: ElementId): void {
    this.linkParent(parent, child);
    this.hayate.element_insert_before(
      parent as number,
      child as number,
      before as number,
    );
  }

  removeChild(_parent: ElementId, child: ElementId): void {
    this.hayate.element_remove(child as number);
    this.pruneLocalSubtree(child);
  }

  setStyle(id: ElementId, style: StylePatch): void {
    const { props, unsetKinds } = stylePatchToMutation(style);
    if (props.length > 0) {
      this.hayate.element_set_style(id as number, props);
    }
    if (unsetKinds.length > 0) {
      this.hayate.element_unset_style(id as number, unsetKinds);
    }
  }

  setText(id: ElementId, text: string): void {
    this.hayate.element_set_text(id as number, text);
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

  private readonly frame = (timestampMs: number): void => {
    this.hayate.render(timestampMs);
    this.dispatchEvents(this.hayate.poll_events());
    this.frameHandle = this.requestFrame(this.frame);
  };

  private dispatchEvents(events: HayateEvent[]): void {
    for (const event of events) {
      switch (event.type) {
        case 'click':
        case 'text-input':
        case 'key-down':
        case 'focus':
        case 'blur':
        case 'hover-enter':
        case 'hover-leave':
          this.dispatchOne(
            event.type === 'text-input'
              ? 'input'
              : event.type === 'key-down'
              ? 'keydown'
              : event.type,
            asElementId(event.target),
            event.type === 'text-input'
              ? { value: event.text }
              : event.type === 'key-down'
              ? { key: event.key }
              : undefined,
          );
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
