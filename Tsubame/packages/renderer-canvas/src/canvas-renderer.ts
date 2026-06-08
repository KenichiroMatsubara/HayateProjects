import type {
  ElementId,
  ElementKind,
  EventHandler,
  EventKind,
  IRenderer,
  PseudoStyleKey,
  StylePatch,
  Unsubscribe,
} from '@tsubame/renderer-protocol';
import { asElementId } from '@tsubame/renderer-protocol';
import type { RawHayate } from './hayate.js';
import { HayateMutationPacket } from './hayate-mutation-packet.js';
import { HAYATE_LISTENER_KIND, parseDelivery, toInteractionEvent } from '@tsubame/protocol-generated/delivery';
import { syncEditContextBounds } from './edit-context-sync.js';

export interface CanvasRendererOptions {
  requestFrame?: (cb: FrameRequestCallback) => number;
  cancelFrame?: (handle: number) => void;
  canvas?: HTMLCanvasElement;
}

interface ListenerEntry {
  handler: EventHandler;
  elementId: ElementId;
}

export class CanvasRenderer implements IRenderer {
  private readonly raw: RawHayate;
  /** Hayate-issued listener id → host handler (ADR-0053). */
  private readonly listeners = new Map<number, ListenerEntry>();
  private nextId = 1;

  private readonly packet = new HayateMutationPacket();

  private readonly canvas: HTMLCanvasElement | null;
  private readonly requestFrame: (cb: FrameRequestCallback) => number;
  private readonly cancelFrame: (handle: number) => void;
  private frameHandle: number | null = null;

  constructor(raw: RawHayate, options: CanvasRendererOptions = {}) {
    this.raw = raw;
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
    this.packet.enqueueAppendChild(parent, child);
  }

  insertBefore(parent: ElementId, child: ElementId, before: ElementId): void {
    this.packet.enqueueInsertBefore(parent, child, before);
  }

  removeChild(_parent: ElementId, child: ElementId): void {
    this.packet.enqueueRemove(child);
  }

  setStyle(id: ElementId, style: StylePatch): void {
    this.packet.enqueueSetStyle(id, style);
  }

  setPseudoStyle(id: ElementId, pseudo: PseudoStyleKey, style: StylePatch): void {
    this.packet.enqueueSetPseudoStyle(id, pseudo, style);
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
    const hayateKind = HAYATE_LISTENER_KIND[event];
    if (hayateKind === undefined) {
      return () => {};
    }

    const listenerId = this.raw.register_listener(id, hayateKind);
    this.listeners.set(listenerId, { handler, elementId: id });
    return () => {
      this.listeners.delete(listenerId);
    };
  }

  /** Drain the ordered mutation packet into the Hayate WASM boundary. */
  private flush(): void {
    this.packet.flush(this.raw);
  }

  private dispatchDeliveries(rows: unknown[]): void {
    for (const row of rows) {
      const { listenerId, event } = parseDelivery(row as unknown[]);
      const entry = this.listeners.get(listenerId);
      if (entry === undefined) continue;
      const interaction = toInteractionEvent(event);
      if (interaction !== null) {
        entry.handler(interaction);
      }
    }
  }

  private readonly frame = (timestampMs: number): void => {
    this.flush();
    this.raw.render(timestampMs);
    if (this.canvas !== null) {
      syncEditContextBounds(this.canvas, this.raw);
    }
    this.dispatchDeliveries(this.raw.poll_events());
    this.frameHandle = this.requestFrame(this.frame);
  };
}
