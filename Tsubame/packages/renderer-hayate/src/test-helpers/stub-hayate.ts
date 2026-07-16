import type { RawHayate } from '../hayate.js';

/**
 * `HayateRenderer` を実 WASM なしに駆動する軽量 fake。`HayateRenderer` に対する
 * 単体テストと、他パッケージ（tsubame-react 等）が自分の `IRenderer` 契約を実
 * `HayateRenderer` に対して検証する統合テストの双方で使う唯一の実装。
 */
export class StubHayate implements RawHayate {
  mutations: Array<{ ops: number[]; styles: number[]; texts: string[]; draws: number[] }> = [];
  renders: number[] = [];
  committedFrames: number[] = [];
  abortedFrames: number[] = [];
  events: unknown[][] = [];
  listenerSeq = 1;
  registeredListeners: Array<{ elementId: number; eventKind: number; listenerId: number }> = [];
  textContentCalls: Array<[number, string]> = [];
  textCalls: Array<[number, string]> = [];
  srcCalls: Array<[number, string]> = [];
  disabledCalls: Array<[number, boolean]> = [];
  pseudoStyleCalls: Array<[number, number, number[]]> = [];

  element_create(): void {}
  set_root(): void {}
  element_set_text(id: number, text: string): void {
    this.textCalls.push([id, text]);
  }
  element_set_text_content(id: number, text: string): void {
    this.textContentCalls.push([id, text]);
  }
  element_set_src(id: number, url: string): void {
    this.srcCalls.push([id, url]);
  }
  element_set_disabled(id: number, disabled: boolean): void {
    this.disabledCalls.push([id, disabled]);
  }
  element_get_text(): string {
    return '';
  }
  element_append_child(): void {}
  element_insert_before(): void {}
  element_remove(): void {}
  element_subtree_ids(): Float64Array {
    return new Float64Array();
  }
  element_set_style(): void {}
  element_set_pseudo_style(id: number, state: number, packed: Float32Array): void {
    this.pseudoStyleCalls.push([id, state, Array.from(packed)]);
  }
  apply_mutations(ops: Float64Array, styles: Float32Array, texts: string[], draws: Float32Array): void {
    this.mutations.push({
      ops: Array.from(ops),
      styles: Array.from(styles),
      texts: Array.from(texts),
      draws: Array.from(draws),
    });
  }
  on_pointer_move(): void {}
  on_pointer_down(): void {}
  on_pointer_up(): void {}
  on_wheel(): void {}
  on_key_down(): void {}
  dispatch_edit_intent(): number { return 1; }
  has_selection(): boolean {
    return false;
  }
  on_text_input(): void {}
  element_get_bounds(): number[] {
    return [0, 0, 0, 0];
  }
  element_effective_visual(): null {
    return null;
  }
  poll_accessibility(): string | null {
    return null;
  }
  /** ADR-0126: 継続すべき pending visual work があるか。テストは描画後の継続要否をこのフラグで制御する。 */
  pendingVisualWork = false;
  has_pending_visual_work(): boolean {
    return this.pendingVisualWork;
  }
  prepare_frame(timestampMs: number): unknown[] {
    this.renders.push(timestampMs);
    const current = this.events;
    this.events = [];
    return [this.renders.length, ...current];
  }
  commit_frame(frameId: number): void {
    this.committedFrames.push(frameId);
  }
  abort_frame(frameId: number): void {
    this.abortedFrames.push(frameId);
  }
  render(timestampMs: number): void {
    this.renders.push(timestampMs);
  }
  poll_events(): unknown[] {
    const current = this.events;
    this.events = [];
    return current;
  }
  register_listener(elementId: number, eventKind: number): number {
    const listenerId = this.listenerSeq++;
    this.registeredListeners.push({ elementId, eventKind, listenerId });
    return listenerId;
  }
  set_background_color(): void {}
  set_tuning(): void {}
  /** ADR-0080/0126: Platform Adapter が入力到着で idle ループを起こすための wake コールバック。 */
  requestRedraw: (() => void) | null = null;
  set_request_redraw(cb: () => void): void {
    this.requestRedraw = cb;
  }
}

/** `HayateRenderer` の frame ループを手動で武装・発火するテスト用スケジューラ。 */
export function manualScheduler() {
  let pending: FrameRequestCallback | null = null;
  return {
    requestFrame: (cb: FrameRequestCallback) => {
      pending = cb;
      return 1;
    },
    cancelFrame: () => {
      pending = null;
    },
    tick: (timestamp = 16) => {
      const cb = pending;
      pending = null;
      cb?.(timestamp);
    },
  };
}
