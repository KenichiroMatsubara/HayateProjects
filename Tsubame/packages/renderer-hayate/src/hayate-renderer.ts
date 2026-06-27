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

/**
 * host-blind コアの構築入力（#476, ADR-0004）。`raw` は Hayate ランタイムの
 * ポート、`requestFrame`/`cancelFrame` は host が確立した frame-clock。これだけ。
 * surface（canvas）・resize・IME・pointer は host 側 adapter が所有するので、
 * platform 識別子（`HTMLCanvasElement` 型・`devicePixelRatio`・`ResizeObserver`・
 * RAF 既定）はここに存在しない。clock 源の確立は host bootstrap の責務。
 */
export interface HayateRendererOptions {
  raw: RawHayate;
  requestFrame: (cb: FrameRequestCallback) => number;
  cancelFrame: (handle: number) => void;
}

interface ListenerEntry {
  handler: EventHandler;
  elementId: ElementId;
}

export class HayateRenderer implements IRenderer {
  private readonly raw: RawHayate;
  /** Hayate が発行したリスナ id → ホストのハンドラ（ADR-0053）。 */
  private readonly listeners = new Map<number, ListenerEntry>();
  private nextId = 1;

  private readonly packet = new HayateMutationPacket();

  private readonly requestFrame: (cb: FrameRequestCallback) => number;
  private readonly cancelFrame: (handle: number) => void;
  private frameHandle: number | null = null;

  constructor(options: HayateRendererOptions) {
    this.raw = options.raw;
    this.requestFrame = options.requestFrame;
    this.cancelFrame = options.cancelFrame;
    // 構築≠開始：コンストラクタは副作用なし。frame ループは明示 start() でしか
    // 走らない（native は構築後 vsync 準備ができてから開始する, #476）。
  }

  /** frame ループを武装する。host が clock の準備を終えてから呼ぶ。冪等。 */
  start(): void {
    if (this.frameHandle === null) {
      this.frameHandle = this.requestFrame(this.frame);
    }
  }

  stop(): void {
    if (this.frameHandle !== null) {
      this.cancelFrame(this.frameHandle);
      this.frameHandle = null;
    }
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
