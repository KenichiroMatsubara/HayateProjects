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

export type ResizeObserverFactory = new (
  callback: ResizeObserverCallback,
) => ResizeObserver;

export interface CanvasRendererOptions {
  requestFrame?: (cb: FrameRequestCallback) => number;
  cancelFrame?: (handle: number) => void;
  canvas?: HTMLCanvasElement;
  /**
   * `false` で ResizeObserver を付けない（埋め込みホストは手動でリサイズする）。
   * `canvas` 指定時は既定で `true`。
   */
  autoResize?: boolean;
  /** テスト用に注入する ResizeObserver コンストラクタ。 */
  createResizeObserver?: ResizeObserverFactory;
  /** テスト用の `devicePixelRatio` 上書き。既定は `globalThis.devicePixelRatio ?? 1`。 */
  devicePixelRatio?: number;
}

interface ListenerEntry {
  handler: EventHandler;
  elementId: ElementId;
}

export class CanvasRenderer implements IRenderer {
  private readonly raw: RawHayate;
  /** Hayate が発行したリスナ id → ホストのハンドラ（ADR-0053）。 */
  private readonly listeners = new Map<number, ListenerEntry>();
  private nextId = 1;

  private readonly packet = new HayateMutationPacket();

  private readonly canvas: HTMLCanvasElement | null;
  private readonly requestFrame: (cb: FrameRequestCallback) => number;
  private readonly cancelFrame: (handle: number) => void;
  /** DPR の明示上書き（テスト/埋め込みホスト）。未設定なら毎リサイズで実時の
   * `globalThis.devicePixelRatio` を読む。モバイル Chrome は構築後に DPR を変える
   * （入力中のソフトキーボード/フォーカスズーム）ため、構築時にキャッシュすると
   * バッキングストアが小さすぎて生成され、シーンが拡大されてグリフが粗くなる。 */
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

  /** 次のリサイズに使う DPR を決める。明示上書きがあればそれを、なければ実時の
   * グローバル値（毎回読み直し、キャッシュしない）。 */
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
    // 共有のスペック生成ディスパッチ（ADR-0008）。Canvas アダプタは enqueue 効果
    // ハンドラだけを埋め、op 種別の分岐はプロトコル側に一度だけ存在する。
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

  /** 順序付きミューテーションパケットを Hayate WASM 境界へ流し込む。 */
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
        // `input` の `value` はワイヤ配信が運ぶ要素の現在値全体（core が
        // `Event::TextInput` に display_text を載せる、ADR-0069 / #474）。以前は
        // 断片しか来ず `element_get_text_content` で読み戻していたが、その経路は
        // 撤去した（IME 配線はアダプタ内で完結し、ホストは RawHayate に IME/読み戻し
        // メソッドを持たない）。
        entry.handler(interaction);
      }
    }
  }

  private readonly frame = (timestampMs: number): void => {
    this.flush();
    // IME（EditContext 着脱・preedit・候補窓 rect）は hayate-adapter-web が
    // `render()` 内で自己配線・自己同期する（ADR-0069）。ホストは IME 経路に関与しない。
    this.raw.render(timestampMs);
    this.dispatchDeliveries(this.raw.poll_events());
    this.frameHandle = this.requestFrame(this.frame);
  };
}
