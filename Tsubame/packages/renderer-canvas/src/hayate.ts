/**
 * Canvas Renderer が依存する Hayate WASM の最小バインディング契約。
 *
 * Tsubame と Hayate の結合点はこのインターフェース（`apply_mutations` の仕様）のみ。
 * 実体の wasm-bindgen エクスポートをこの形に適合させる。
 * テストやデモでは同形の JS スタブを差し込める。
 */
export interface HayateWasm {
  /**
   * フレーム分の mutation を 1 回/frame で適用する hot path。
   * @param ops    固定長レコードの ops ストリーム（ADR-0003）
   * @param styles OP_SET_STYLE が参照する TAG エンコード済み f32 バッファ
   */
  apply_mutations(ops: Float64Array, styles: Float32Array): void;

  /**
   * 文字列 op は typed array に収まらず頻度も低いため、バッチ外の個別呼び出し
   * とする（ADR-0003）。
   */
  element_set_text(id: number, text: string): void;

  /**
   * 蓄積された Interaction Event を `Array<Array<any>>` で返す（ADR-0034）。
   *
   * 各サブ配列は `[kind: number, ...fields]` の形式。
   * 文字列ペイロード（text_input / key_down 等）を運ぶために
   * フラット配列ではなく Array<Array<any>> を採用（フラット f64 では文字列不可）。
   *
   * Rust 側の event_kind_*() 定数と対応するコード体系:
   *   click=0, focus=1, blur=2, hover-enter=10, hover-leave=11 ほか
   */
  poll_events(): Array<Array<number | string>>;
}
