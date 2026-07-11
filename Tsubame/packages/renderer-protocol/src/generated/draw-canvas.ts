// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと
// 生成元: @torimi/hayate-protocol-spec（draw_ops / draw_paint_fields）
//
// painter（`draw` property の値）が受け取る記録面の構造的インターフェース
//（#730 / ADR-0141）。実体は Hayate Renderer では wire 記録の recorder Canvas
//（@torimi/tsubame-protocol-generated/recorder）、DOM Renderer では canvas 2D への
// replay（Tsubame ADR-0014）。recorder と同じ op 表から生成するため、spec に
// op が増えても手書き修正なしで painter の型が追随する（Script Adapter 規律）。
// `finish()` はフレームワーク側の口なので意図的に含めない（painter はバッファに
// 触れない）。

/** 記録済みパスの最小 surface（recorder `Path` が満たす）。 */
export interface DrawRecordedPath {
  /** 記録済み op 列（再生用の読み取り専用ビュー）。 */
  record(): readonly number[];
}

/** Paint を wire 解決した tagged パケット（codec `DrawPaint` と同形）。 */
export interface DrawPaintPacket {
  readonly color?: readonly [number, number, number, number];
  readonly fillRule?: number;
  readonly strokeWidth?: number;
  readonly cap?: number;
  readonly join?: number;
  readonly miterLimit?: number;
  readonly dash?: readonly number[];
  readonly dashOffset?: number;
}

/** `drawPath` に渡す Paint の最小 surface（recorder `Paint` が満たす）。 */
export interface DrawPaintSource {
  /** PaintingStyle（0 = fill, 1 = stroke）。 */
  readonly style: number;
  /** 現在のフィールドを wire パケットへ解決する（不正値はエラー）。 */
  toDrawPaint(): DrawPaintPacket;
}

/**
 * painter の記録面。Flutter/Skia 流ステートレス設計: canvas 自体の状態は
 * save/restore の変換・クリップスタックのみ。座標はボーダーボックス左上原点・
 * 論理 px・DPR 不可視（ADR-0141）。
 */
export interface DrawCanvas {
  /** `path` を `paint` で塗る / 輪郭描画する（paint.style で分岐）。 */
  drawPath(path: DrawRecordedPath, paint: DrawPaintSource): this;

  /** 以降の描画を `path` で切り抜く（対応する restore で解除）。 */
  clipPath(path: DrawRecordedPath): this;

  save(): this;

  restore(): this;

  translate(dx: number, dy: number): this;

  rotate(radians: number): this;

  scale(sx: number, sy: number): this;

  transform(a: number, b: number, c: number, d: number, e: number, f: number): this;

  clipRect(x: number, y: number, width: number, height: number): this;

}
