import type {
  ElementId,
  ElementKind,
  EventHandler,
  EventKind,
  IRenderer,
  PseudoStyleKey,
  StylePatch,
  Unsubscribe,
  ViewportCondition,
} from '@tsubame/renderer-protocol';
import {
  asElementId,
  assertKnownElementProperty,
  coerceElementProperty,
  dispatchElementPropertyOp,
} from '@tsubame/renderer-protocol';
import type { RawHayate } from './hayate.js';
import { HayateMutationPacket } from './hayate-mutation-packet.js';
import { HAYATE_LISTENER_KIND, parseDelivery, toInteractionEvent } from '@tsubame/protocol-generated/delivery';
import { syncEditContext } from './edit-context-sync.js';

export type ResizeObserverFactory = new (
  callback: ResizeObserverCallback,
) => ResizeObserver;

export interface CanvasRendererOptions {
  requestFrame?: (cb: FrameRequestCallback) => number;
  cancelFrame?: (handle: number) => void;
  canvas?: HTMLCanvasElement;
  /**
   * When `false`, skip attaching a ResizeObserver (embedded hosts resize manually).
   * Defaults to `true` when `canvas` is set.
   */
  autoResize?: boolean;
  /** Injectable ResizeObserver constructor for tests. */
  createResizeObserver?: ResizeObserverFactory;
  /** Override `devicePixelRatio` (tests). Defaults to `globalThis.devicePixelRatio ?? 1`. */
  devicePixelRatio?: number;
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
  /** Explicit DPR override (tests/embedded hosts). When unset the observer reads
   * the live `globalThis.devicePixelRatio` on every resize — mobile Chrome bumps
   * it after construction (soft-keyboard / zoom-on-focus while typing), and a
   * value cached at construction would rebuild the backing store too small and
   * upscale the scene, roughening glyphs. */
  private readonly devicePixelRatioOverride: number | undefined;
  private resizeObserver: ResizeObserver | null = null;
  private frameHandle: number | null = null;

  constructor(raw: RawHayate, options: CanvasRendererOptions = {}) {
    this.raw = raw;
    this.canvas = options.canvas ?? null;
    this.requestFrame =
      options.requestFrame ?? globalThis.requestAnimationFrame.bind(globalThis);
    this.cancelFrame =
      options.cancelFrame ?? globalThis.cancelAnimationFrame.bind(globalThis);
    this.devicePixelRatioOverride = options.devicePixelRatio;

    const autoResize = options.autoResize ?? this.canvas !== null;
    if (this.canvas !== null && autoResize) {
      this.attachResizeObserver(this.canvas, options.createResizeObserver);
    }

    this.frameHandle = this.requestFrame(this.frame);
  }

  stop(): void {
    if (this.frameHandle !== null) {
      this.cancelFrame(this.frameHandle);
      this.frameHandle = null;
    }
    this.resizeObserver?.disconnect();
    this.resizeObserver = null;
  }

  private attachResizeObserver(
    canvas: HTMLCanvasElement,
    createResizeObserver?: ResizeObserverFactory,
  ): void {
    const ResizeObserverCtor =
      createResizeObserver ??
      (typeof globalThis.ResizeObserver !== 'undefined'
        ? globalThis.ResizeObserver
        : undefined);
    if (ResizeObserverCtor === undefined) {
      return;
    }

    const syncFromContentBox = (width: number, height: number): void => {
      this.resize(Math.round(width), Math.round(height), this.currentDevicePixelRatio());
    };

    const rect = canvas.getBoundingClientRect();
    syncFromContentBox(rect.width, rect.height);

    const observer = new ResizeObserverCtor((entries) => {
      const entry = entries[0];
      if (entry === undefined) return;
      const { width, height } = entry.contentRect;
      syncFromContentBox(width, height);
    });
    observer.observe(canvas);
    this.resizeObserver = observer;
  }

  /** Resolve the device pixel ratio for the next resize: the explicit override
   * when given, otherwise the *live* global (re-read each call, never cached). */
  private currentDevicePixelRatio(): number {
    return this.devicePixelRatioOverride ?? globalThis.devicePixelRatio ?? 1;
  }

  resize(width: number, height: number, scale = 1): void {
    const dpr = Math.max(1, scale);
    if (this.canvas !== null) {
      this.canvas.width = Math.round(width * dpr);
      this.canvas.height = Math.round(height * dpr);
    }
    this.raw.on_resize(width, height, dpr);
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

  setStyleVariant(id: ElementId, condition: ViewportCondition, style: StylePatch): void {
    this.packet.enqueueSetStyleVariant(id, condition, style);
  }

  setText(id: ElementId, text: string): void {
    this.packet.enqueueSetText(id, text);
  }

  setProperty(id: ElementId, name: string, value: unknown): void {
    assertKnownElementProperty(name);
    const op = coerceElementProperty(name, value);
    // Shared spec-generated dispatch (ADR-0008): the Canvas adapter fills only the
    // enqueue effect handlers — the op-kind match lives once in the protocol.
    dispatchElementPropertyOp<void>(op, {
      'text-content': ({ text }) => this.packet.enqueueSetTextContent(id, text),
      placeholder: ({ text }) => this.packet.enqueueSetText(id, text),
      src: ({ text }) => this.packet.enqueueSetSrc(id, text),
      disabled: ({ disabled }) => this.packet.enqueueSetDisabled(id, disabled),
      'user-select': ({ value }) => this.packet.enqueueSetUserSelect(id, value),
      multiline: ({ multiline }) => this.packet.enqueueSetMultiline(id, multiline),
    });
  }

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
        // The text_input wire payload carries only the freshly inserted
        // fragment, but `InteractionEvent.value` is contractually the element's
        // *current* value (the DOM renderer reads `target.value`). Read the
        // authoritative content back from the tree so controlled inputs see the
        // whole string, not just the last keystroke.
        if (interaction.kind === 'input') {
          interaction.value = this.raw.element_get_text_content(interaction.target);
        }
        entry.handler(interaction);
      }
    }
  }

  private readonly frame = (timestampMs: number): void => {
    this.flush();
    this.raw.render(timestampMs);
    if (this.canvas !== null) {
      syncEditContext(this.canvas, this.raw);
    }
    this.dispatchDeliveries(this.raw.poll_events());
    this.frameHandle = this.requestFrame(this.frame);
  };
}
