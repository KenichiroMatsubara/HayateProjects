import type {
  DrawProperty,
  DrawSize,
  ElementId,
  ElementKind,
  EventHandler,
  EventKind,
  IRenderer,
  PseudoStyleKey,
  StylePatch,
  Unsubscribe,
  ViewportCondition,
} from '@torimi/tsubame-renderer-protocol';
import {
  asElementId,
  assertKnownElementProperty,
  coerceElementProperty,
  dispatchElementPropertyOp,
  drawNeedsRepaint,
  invokePainter,
} from '@torimi/tsubame-renderer-protocol';
import type { RawHayate } from './hayate.js';
import { HayateMutationPacket } from './hayate-mutation-packet.js';
import { EVENT_KIND } from '@torimi/tsubame-protocol-generated/protocol';
import { Canvas } from '@torimi/tsubame-protocol-generated/recorder';
import { HAYATE_LISTENER_KIND, parseDelivery, toInteractionEvent } from '@torimi/tsubame-protocol-generated/delivery';

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

/** draw property の要素ごとの状態。`size` はレイアウト確定前は null（ADR-0143）。 */
interface DrawState {
  value: DrawProperty;
  size: DrawSize | null;
}

export class HayateRenderer implements IRenderer {
  private readonly raw: RawHayate;
  /** Hayate が発行したリスナ id → ホストのハンドラ（ADR-0053）。 */
  private readonly listeners = new Map<number, ListenerEntry>();
  /** draw property を持つ要素の状態（#730）。 */
  private readonly drawStates = new Map<ElementId, DrawState>();
  /** 内部購読した layout size イベント（#725）のリスナ id → 要素 id。 */
  private readonly drawListeners = new Map<number, ElementId>();
  private nextId = 1;

  private readonly packet = new HayateMutationPacket();

  private readonly requestFrame: (cb: FrameRequestCallback) => number;
  private readonly cancelFrame: (handle: number) => void;
  private frameHandle: number | null = null;
  /** start() 後だけ wake を許す（構築≠開始, #476）。stop() で false に戻す。 */
  private started = false;

  constructor(options: HayateRendererOptions) {
    this.raw = options.raw;
    this.requestFrame = options.requestFrame;
    this.cancelFrame = options.cancelFrame;
    // 構築≠開始：コンストラクタは副作用なし。frame ループは明示 start() でしか
    // 走らない（native は構築後 vsync 準備ができてから開始する, #476）。
  }

  /** frame ループを武装する。host が clock の準備を終えてから呼ぶ。冪等。
   * これ自体が冷間始動の wake 入口で、以後は継続 pending / mutation 到着で再武装する。 */
  start(): void {
    this.started = true;
    // ADR-0080/0126: 入力到着（ポインタ / 編集）を wake 源として配線する。web adapter は
    // 自前配線した listener で入力を Rust 側にバッファするだけなので、idle に落ちたループを
    // JS の scheduleFrame で起こさないと drain されない（Android Chrome でタップが無反応に
    // なる回帰の修正）。scheduleFrame は冪等・started ゲート付きなので二重武装しない。入力
    // ingress を持たない front（set_request_redraw 未実装）では no-op。
    this.raw.set_request_redraw?.(() => this.scheduleFrame());
    this.scheduleFrame();
  }

  stop(): void {
    this.started = false;
    if (this.frameHandle !== null) {
      this.cancelFrame(this.frameHandle);
      this.frameHandle = null;
    }
  }

  /**
   * 次フレームを 1 枚だけ要求する（ADR-0126 の唯一の wake 入口）。既に武装済み／
   * 未 start なら何もしない（冪等）。start・継続 pending・mutation 到着のいずれの
   * 経路もここを通り、idle ループの二重武装を防ぐ。
   */
  private scheduleFrame(): void {
    if (this.started && this.frameHandle === null) {
      this.frameHandle = this.requestFrame(this.frame);
    }
  }

  createElement(kind: ElementKind): ElementId {
    const id = asElementId(this.nextId++);
    this.packet.enqueueCreateElement(id, kind);
    this.scheduleFrame();
    return id;
  }

  setRoot(id: ElementId): void {
    this.packet.enqueueSetRoot(id);
    this.scheduleFrame();
  }

  appendChild(parent: ElementId, child: ElementId): void {
    this.packet.enqueueAppendChild(parent, child);
    this.scheduleFrame();
  }

  insertBefore(parent: ElementId, child: ElementId, before: ElementId): void {
    this.packet.enqueueInsertBefore(parent, child, before);
    this.scheduleFrame();
  }

  removeChild(_parent: ElementId, child: ElementId): void {
    this.packet.enqueueRemove(child);
    this.scheduleFrame();
  }

  setStyle(id: ElementId, style: StylePatch): void {
    this.packet.enqueueSetStyle(id, style);
    this.scheduleFrame();
  }

  setPseudoStyle(id: ElementId, pseudo: PseudoStyleKey, style: StylePatch): void {
    this.packet.enqueueSetPseudoStyle(id, pseudo, style);
    this.scheduleFrame();
  }

  setStyleVariant(id: ElementId, condition: ViewportCondition, style: StylePatch): void {
    this.packet.enqueueSetStyleVariant(id, condition, style);
    this.scheduleFrame();
  }

  setText(id: ElementId, text: string): void {
    this.packet.enqueueSetText(id, text);
    this.scheduleFrame();
  }

  /**
   * `view` の draw property（painter・#730 / ADR-0141）。wire 経路はレイアウト確定
   * サイズを同期では知れないため、per-element layout size イベント（#725）を内部購読し、
   * 受信時（初回確定・サイズ変化）に painter を実サイズで呼んで display list を記録、
   * 次フレームの mutation で `draws` チャネルに載せる（1 フレーム遅延は仕様・ADR-0143）。
   */
  setDraw(id: ElementId, value: DrawProperty | null): void {
    const state = this.drawStates.get(id);
    if (value === null) {
      if (state === undefined) return;
      this.drawStates.delete(id);
      this.packet.enqueueSetDraw(id, []);
      this.scheduleFrame();
      return;
    }
    if (state === undefined) {
      const listenerId = this.raw.register_listener(
        id as unknown as number,
        EVENT_KIND.LAYOUT_RESIZE,
      );
      this.drawListeners.set(listenerId, id);
      this.drawStates.set(id, { value, size: null });
      return;
    }
    const repaint = drawNeedsRepaint(value, state.value);
    state.value = value;
    if (repaint && state.size !== null) {
      this.recordDraw(id, state);
    }
  }

  /** painter を現サイズで走らせ、記録した display list を次フレームの mutation に積む。 */
  private recordDraw(id: ElementId, state: DrawState): void {
    const canvas = new Canvas();
    invokePainter(state.value, canvas, state.size!);
    this.packet.enqueueSetDraw(id, canvas.finish());
    this.scheduleFrame();
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
    this.scheduleFrame();
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
      // per-element layout size イベント（#725）は adapter へは配らず、draw の
      // paint タイミング源として renderer 内で消費する（wireRole: hayate-internal）。
      if (event.kind === 'layout_resize') {
        this.onLayoutResize(listenerId, event.width, event.height);
        continue;
      }
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

  /** レイアウト確定・サイズ変化の通知で painter を実サイズで走らせる（#730）。 */
  private onLayoutResize(listenerId: number, width: number, height: number): void {
    const id = this.drawListeners.get(listenerId);
    if (id === undefined) return;
    const state = this.drawStates.get(id);
    if (state === undefined) return;
    // core は size 非変化 commit では発火しない（#725）が、同サイズ通知を無駄な
    // 再記録・再送信に増幅しないよう renderer 側でも守る。
    if (state.size !== null && state.size.width === width && state.size.height === height) {
      return;
    }
    state.size = { width, height };
    this.recordDraw(id, state);
  }

  private readonly frame = (timestampMs: number): void => {
    // このコールバックは消費された。継続 pending があるときだけ末尾で再武装する
    // （無条件の自己再スケジュールを撤廃, ADR-0126）。
    this.frameHandle = null;
    const [rawFrameId, ...deliveries] = this.raw.prepare_frame(timestampMs);
    if (typeof rawFrameId !== 'number' || !Number.isSafeInteger(rawFrameId)) {
      this.packet.discard();
      throw new TypeError('Hayate prepare_frame returned an invalid frame id');
    }
    try {
      this.dispatchDeliveries(deliveries);
      // prepare が drain した delivery の handler mutation まで、この matching commit
      // より前に同じ packet として境界へ流す（ADR-0151 / #827）。
      this.flush();
    } catch (error) {
      this.packet.discard();
      this.raw.abort_frame(rawFrameId);
      throw error;
    }
    // commit の execution failure は AppHost 側ですでに transaction を終端している。
    // abort を重ねて元の型付き failure を上書きしない。
    this.raw.commit_frame(rawFrameId);
    // ADR-0126: idle（visual_dirty 空）では次フレームを出さない。継続すべき pending
    // （進行中 transition / カーソル点滅 / スクロール物理）があるときだけ再武装する。
    // delivery ハンドラが mutation を積んだ場合は dispatchDeliveries→scheduleFrame で
    // 既に武装済みのこともある（scheduleFrame は冪等）。
    if (this.raw.has_pending_visual_work()) {
      this.scheduleFrame();
    }
  };
}
