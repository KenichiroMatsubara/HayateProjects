/**
 * Hayate WASM レンダラ（`HayateElementRenderer`）が公開する element-layer メソッドの
 * うち、Tsubame の host-blind `HayateRenderer` が駆動に必要とする最小 surface。
 *
 * これは host（producer）側が所有する「生成物の契約」である。Tsubame は自前の
 * `RawHayate` ポート（consumer 側の契約）を別に持ち、両者は App（合成ルート）で
 * 構造的に出会う。Hayate→Tsubame の依存は張らない（CONTEXT-MAP の依存境界）。
 *
 * wasm-bindgen 生成クラスは構造的にこれを充足するので、`createHayateWebHost` は
 * 生成インスタンスをそのまま `RawHayate` として返せる。
 */
export interface RawHayate {
  element_create(id: number, kind: number): void;
  set_root(id: number): void;
  element_append_child(parent: number, child: number): void;
  element_insert_before(parent: number, child: number, before: number): void;
  element_remove(id: number): void;
  element_get_text(id: number): string;
  element_get_bounds(id: number): Float32Array | number[];
  element_subtree_ids(id: number): Float64Array;
  apply_mutations(ops: Float64Array, styles: Float32Array, texts: string[], draws: Float32Array): void;
  on_pointer_move(x: number, y: number): void;
  on_pointer_down(x: number, y: number): void;
  on_pointer_up(x: number, y: number): void;
  on_wheel(x: number, y: number, deltaX: number, deltaY: number): void;
  on_key_down(key: string, modifiers: number): void;
  has_selection(): boolean;
  on_text_input(id: number, text: string): void;
  poll_accessibility(): string | null;
  render(timestampMs: number): void;
  /** ADR-0126: 直近の `render()` 後に継続すべき pending visual work（進行中 transition /
   * カーソル点滅 / スクロール物理 = `visual_dirty`）が残るか。consumer（`@tsubame/
   * renderer-hayate` の `RawHayate` ポート）はこれを必須とする — web（canvas.rs）/
   * native（js_host.rs）とも実装済みで、生成物の契約にも載る。 */
  has_pending_visual_work(): boolean;
  poll_events(): unknown[];
  register_listener(element_id: number, event_kind: number): number;
  set_background_color(r: number, g: number, b: number): void;
  /** 開発専用: `tuning.json` の味付け定数オーバーライドを重ねる。不正な JSON や
   * 未知のキーで throw するが、host は握りつぶしコンパイル済み既定を残す。 */
  set_tuning(json: string): void;
  element_effective_visual(id: number): HayateEffectiveVisual | null;
  /** ADR-0080/0126 の Android 延長: host の frame ループが armed になるたびに
   * native へ知らせる（`@tsubame/renderer-hayate` の `RawHayate` と同型）。native.ts の
   * `requestFrame` がこれを叩く。Web ホストでは実装しないため optional。 */
  request_pump?(): void;
}

/** `{ r, g, b, a }`（0..1）。生成 codec の `HayateColorRecord` と構造的に同一。 */
export interface HayateColorRecord {
  r: number;
  g: number;
  b: number;
  a: number;
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
