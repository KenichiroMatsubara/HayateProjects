/**
 * Canvas Renderer が依存する Hayate WASM の最小バインディング契約。
 *
 * Tsubame と Hayate は独立リポジトリであり、結合点はこのインターフェース
 * （`apply_mutations` の仕様）のみ（CONTEXT.md / ADR-0001）。実体の wasm-bindgen
 * エクスポートをこの形に適合させる。テストやデモでは同形の JS スタブを差し込める。
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
   * 蓄積された Interaction Event を `[kind_code, element_id]` の繰り返しで返す。
   * 呼び出しでキューは消費される。
   * Hayate WASM は Array<number> を返す（wasm-bindgen js_sys::Array）ため
   * ArrayLike<number> で受ける。
   */
  poll_events(): ArrayLike<number>;
}
