import type { HayateColorRecord } from '@tsubame/protocol-generated/codec';

// パース/型強制は生成 codec の一箇所だけに置く。既存の `./hayate.js`
// インポータが parseColor/parseDimension を再実装せず動き続けるよう re-export する。
export {
  parseColor,
  parseDimension,
  finiteNumber,
  finiteInteger,
} from '@tsubame/protocol-generated/codec';
export type {
  HayateColorRecord,
  HayateDimensionRecord,
  HayateDimensionUnit,
} from '@tsubame/protocol-generated/codec';

/**
 * 実 Hayate WASM（`HayateElementRenderer`）が公開する WIT element-layer の
 * メソッドのうち、`HayateRenderer` が呼び出すものだけを型付けした最小 interface。
 *
 * wasm-bindgen 生成クラスは構造的にこれを充足するため、`init.ts` では
 * 生成インスタンスをそのまま `RawHayate` として渡せる。スタイルは
 * `Float32Array`（style-packet TAG 形式）、ミューテーションは
 * `apply_mutations(ops, styles, texts)`（ADR-0052）、イベントは delivery
 * poll（ADR-0053: `[listener_id, kind, ...fields]`）でやり取りする。
 */
export interface RawHayate {
  element_create(id: number, kind: number): void;
  set_root(id: number): void;
  element_append_child(parent: number, child: number): void;
  element_insert_before(parent: number, child: number, before: number): void;
  element_remove(id: number): void;
  // 命令的 `element_set_*` セッターは撤去した（#439）。全ミューテーションは
  // `apply_mutations`（ADR-0052 のバッチ経路）1 本を通る。構造系・クエリ・入力・
  // ライフサイクルだけが命令的なまま残る。
  element_get_text(id: number): string;
  /** `layout_cache` 由来の絶対レイアウト境界 `[x, y, width, height]`。 */
  element_get_bounds(id: number): Float32Array | number[];
  /** `id` とその子孫の要素 id（Hayate ElementTree が正）。 */
  element_subtree_ids(id: number): Float64Array;
  apply_mutations(
    ops: Float64Array,
    styles: Float32Array,
    texts: string[],
    draws: Float32Array,
  ): void;
  // viewport 追従（`on_resize`）はこのポートから除いた。Web は hayate-adapter-web の
  // 自己配線 ResizeObserver、Android は native ループが `tree.set_viewport` を直接
  // 駆動する（ADR-0080, native 延長は issue #475）。Tsubame は resize 経路に居ない。
  on_pointer_move(x: number, y: number): void;
  on_pointer_down(x: number, y: number): void;
  on_pointer_up(x: number, y: number): void;
  on_wheel(x: number, y: number, deltaX: number, deltaY: number): void;
  on_key_down(key: string, modifiers: number): void;
  /** document 全体のテキスト選択が有効かどうか（ADR-0097）。 */
  has_selection(): boolean;
  // IME（EditContext 着脱・preedit/commit・候補窓 rect）は hayate-adapter-web が
  // `render()` 内で自己配線・自己同期する（ADR-0069 完成、#474）。ホストは IME 経路に
  // 関与しないため、`on_composition_*` / `ime_*` / `focused_element_id` /
  // `element_get_text_content` はこのポートに存在しない。
  on_text_input(id: number, text: string): void;
  /** JSON エンコードした AccessKit `TreeUpdate`（ADR-0041）。レイアウト前は `null`。 */
  poll_accessibility(): string | null;
  render(timestampMs: number): void;
  /** ADR-0126: 直近の `render()` 後に継続すべき pending visual work（進行中
   * transition / カーソル点滅 / スクロール物理 = `visual_dirty`）が残るか。アダプタは
   * これが真のときだけ次フレームを要求し、偽なら idle に落ちる（毎フレーム自己再
   * スケジュールしない）。 */
  has_pending_visual_work(): boolean;
  /** ADR-0080/0126: 入力到着で on-demand フレームループを冷間始動するための wake
   * コールバックを登録する。Platform Adapter（web adapter）は自前配線したポインタ /
   * 編集 listener が入力をバッファした直後にこれを叩き、idle に落ちたループを 1 フレーム
   * だけ起こして `pending_pointer` / `pending_edit` を drain させる。これが無いと idle 時の
   * タップ・キー入力が drain されず捨てられる（Android Chrome でボタンが無反応になる回帰）。
   *
   * 入力 ingress を持たない front（native ループが入力を直接駆動する Android 等）では
   * 実装されないため optional。`HayateRenderer` は存在すれば `start()` で配線する。 */
  set_request_redraw?(cb: () => void): void;
  /** ADR-0080/0126 の Android 延長: `requestFrame` が呼ばれる（＝この host の
   * frame ループが armed になる）たびに native へ知らせる。Web は `requestFrame` が
   * `window.requestAnimationFrame` に直結するため、armed になれば OS の compositor が
   * 次の vsync で必ず呼び戻してくれる。Android の on-demand ループにはその自走クロックが
   * 無く、native は「入力到着」でしか起きないため、click ハンドラが `setStyle` 等を呼んで
   * `scheduleFrame` が自己再武装した場合（次の `flush` でしか反映されない）、native 側の
   * wake が来ないと二度と pump されず、タップの見た目の反映が永久に止まっていた。
   * 入力 ingress を持たない front（native ループが入力を直接駆動しない Web 等）では
   * 実装されないため optional。 */
  request_pump?(): void;
  poll_events(): unknown[];
  register_listener(element_id: number, event_kind: number): number;
  set_background_color(r: number, g: number, b: number): void;
  /** 開発専用: `tuning.json` の味付け定数オーバーライドを重ねる。不正な JSON や
   * 未知のキーで throw するが、ホストは握りつぶしコンパイル済み既定を残す。
   * ファイル編集 + F5 で再ビルドなしに再適用される。 */
  set_tuning(json: string): void;
  /** 継承 + 擬似状態の解決後のスタイル（ADR-0067）。`id` が未知なら `null`。 */
  element_effective_visual(id: number): HayateEffectiveVisual | null;
}

/** 実効スタイル解決後の `hayate_core::Visual` を JS 向けに写したもの（ADR-0067）。 */
export interface HayateEffectiveVisual {
  backgroundColor: HayateColorRecord | null;
  opacity: number;
  borderRadius: number;
  borderWidth: number;
  borderColor: HayateColorRecord | null;
  textColor: HayateColorRecord | null;
  fontSize: number | null;
  fontWeight: number | null;
  fontStyle: 'normal' | 'italic' | 'oblique' | null;
  textDecoration: 'none' | 'underline' | 'line-through' | null;
  zIndex: number;
  fontFamily: string | null;
}
