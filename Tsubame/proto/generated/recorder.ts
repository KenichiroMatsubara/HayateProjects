// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと
// 生成元: @hayate/protocol-spec（draw_ops / draw_paint_fields）。
// アプリ作者が painter 内で使う Flutter 流ステートレス記録 API（#729 / ADR-0143）。
// Path / Paint は第一級オブジェクト、canvas.drawPath(path, paint) で呼び出しごとに
// paint を明示する。中身は spec の op 表から表駆動で生成した薄い encoder で、
// draws バッファ（f32）へ書き出すだけ（手書きの意味論を持たない）。

/* eslint-disable */
import {
  appendDrawArcTo,
  appendDrawCircle,
  appendDrawClipPath,
  appendDrawClipRect,
  appendDrawClose,
  appendDrawCubicTo,
  appendDrawFill,
  appendDrawLineTo,
  appendDrawMoveTo,
  appendDrawOval,
  appendDrawQuadraticTo,
  appendDrawRect,
  appendDrawRestore,
  appendDrawRotate,
  appendDrawRrect,
  appendDrawSave,
  appendDrawScale,
  appendDrawStroke,
  appendDrawTransform,
  appendDrawTranslate,
  type DrawPaint,
} from './codec.js';
import type { DrawCanvas } from '@tsubame/renderer-protocol';

/** Flutter PaintingStyle（fill = 塗り、stroke = 輪郭）。 */
export enum PaintingStyle {
  fill = 0,
  stroke = 1,
}

/** Flutter StrokeCap（Hayate line_cap enum）。 */
export enum StrokeCap {
  butt = 0,
  round = 1,
  square = 2,
}

/** Flutter StrokeJoin（Hayate line_join enum）。 */
export enum StrokeJoin {
  miter = 0,
  round = 1,
  bevel = 2,
}

/** Flutter PathFillType（Hayate fill_rule enum）。 */
export enum PathFillType {
  nonZero = 0,
  evenOdd = 1,
}

/** ストレート RGBA（各 0..1）。 */
export type Rgba = readonly [number, number, number, number];

/**
 * 描画スタイル。fill / stroke を `style` で切り替える。閉じた語彙（Renderer
 * Protocol）: `toDrawPaint()` は不正な enum 値・範囲外をエラーにする。
 */
export class Paint {
  color: Rgba = [0, 0, 0, 1];
  style: PaintingStyle = PaintingStyle.fill;
  fillType: PathFillType = PathFillType.nonZero;
  strokeWidth = 1;
  strokeCap: StrokeCap = StrokeCap.butt;
  strokeJoin: StrokeJoin = StrokeJoin.miter;
  strokeMiterLimit = 4;
  dash: readonly number[] = [];
  dashOffset = 0;

  /** 現在のフィールドを wire の DrawPaint パケットへ解決する（不正値はエラー）。 */
  toDrawPaint(): DrawPaint {
    assertRgba(this.color);
    assertEnum(PathFillType, this.fillType, "fillType");
    assertEnum(PaintingStyle, this.style, "style");
    const paint: {
      -readonly [K in keyof DrawPaint]: DrawPaint[K];
    } = { color: this.color };
    if (this.fillType !== PathFillType.nonZero) paint.fillRule = this.fillType;
    if (this.style === PaintingStyle.stroke) {
      assertFinite(this.strokeWidth, "strokeWidth", 0);
      assertEnum(StrokeCap, this.strokeCap, "strokeCap");
      assertEnum(StrokeJoin, this.strokeJoin, "strokeJoin");
      assertFinite(this.strokeMiterLimit, "strokeMiterLimit", 0);
      paint.strokeWidth = this.strokeWidth;
      paint.cap = this.strokeCap;
      paint.join = this.strokeJoin;
      paint.miterLimit = this.strokeMiterLimit;
      if (this.dash.length > 0) {
        for (const d of this.dash) assertFinite(d, "dash", 0);
        paint.dash = this.dash;
        assertFinite(this.dashOffset, "dashOffset");
        paint.dashOffset = this.dashOffset;
      }
    }
    return paint;
  }
}

/**
 * 記録済みパス。フレーム間・要素間で再利用できる不変の op 列として保持し、
 * `canvas.drawPath` / `canvas.clipPath` で何度でも再生できる。メソッドは
 * draw_ops の path-verb から生成される（op 追加時に手書き修正は不要）。
 */
export class Path {
  private readonly ops: number[] = [];

  moveTo(x: number, y: number): this {
    appendDrawMoveTo(this.ops, x, y);
    return this;
  }

  lineTo(x: number, y: number): this {
    appendDrawLineTo(this.ops, x, y);
    return this;
  }

  close(): this {
    appendDrawClose(this.ops);
    return this;
  }

  quadraticBezierTo(cx: number, cy: number, x: number, y: number): this {
    appendDrawQuadraticTo(this.ops, cx, cy, x, y);
    return this;
  }

  cubicTo(c1x: number, c1y: number, c2x: number, c2y: number, x: number, y: number): this {
    appendDrawCubicTo(this.ops, c1x, c1y, c2x, c2y, x, y);
    return this;
  }

  arcTo(x1: number, y1: number, x2: number, y2: number, radius: number): this {
    appendDrawArcTo(this.ops, x1, y1, x2, y2, radius);
    return this;
  }

  addRect(x: number, y: number, width: number, height: number): this {
    appendDrawRect(this.ops, x, y, width, height);
    return this;
  }

  addRRect(x: number, y: number, width: number, height: number, rx: number, ry: number): this {
    appendDrawRrect(this.ops, x, y, width, height, rx, ry);
    return this;
  }

  addOval(x: number, y: number, width: number, height: number): this {
    appendDrawOval(this.ops, x, y, width, height);
    return this;
  }

  addCircle(cx: number, cy: number, radius: number): this {
    appendDrawCircle(this.ops, cx, cy, radius);
    return this;
  }

  /** 記録済み op 列（再生用の読み取り専用ビュー。Path は不変）。 */
  record(): readonly number[] {
    return this.ops;
  }
}

/**
 * draws バッファへの記録面。Flutter/Skia 流ステートレス設計: canvas 自体の
 * 状態は save/restore の変換・クリップスタックのみ。座標操作・クリップ矩形の
 * メソッドは draw_ops の構造 command から生成される。painter へは Renderer
 * Protocol の `DrawCanvas`（同じ op 表から生成・#730）として渡り、implements で
 * 型サーフェスとの drift をコンパイル時に検出する。
 */
export class Canvas implements DrawCanvas {
  private readonly buf: number[] = [];

  /** `path` を `paint` で塗る / 輪郭描画する（paint.style で分岐）。 */
  drawPath(path: Path, paint: Paint): this {
    for (const v of path.record()) this.buf.push(v);
    if (paint.style === PaintingStyle.stroke) {
      appendDrawStroke(this.buf, paint.toDrawPaint());
    } else {
      appendDrawFill(this.buf, paint.toDrawPaint());
    }
    return this;
  }

  /** 以降の描画を `path` で切り抜く（対応する restore で解除）。 */
  clipPath(path: Path): this {
    for (const v of path.record()) this.buf.push(v);
    appendDrawClipPath(this.buf);
    return this;
  }

  save(): this {
    appendDrawSave(this.buf);
    return this;
  }

  restore(): this {
    appendDrawRestore(this.buf);
    return this;
  }

  translate(dx: number, dy: number): this {
    appendDrawTranslate(this.buf, dx, dy);
    return this;
  }

  rotate(radians: number): this {
    appendDrawRotate(this.buf, radians);
    return this;
  }

  scale(sx: number, sy: number): this {
    appendDrawScale(this.buf, sx, sy);
    return this;
  }

  transform(a: number, b: number, c: number, d: number, e: number, f: number): this {
    appendDrawTransform(this.buf, a, b, c, d, e, f);
    return this;
  }

  clipRect(x: number, y: number, width: number, height: number): this {
    appendDrawClipRect(this.buf, x, y, width, height);
    return this;
  }

  /** 記録した display list（draws チャネルへ載せる f32 列）。 */
  finish(): number[] {
    return this.buf;
  }
}

function assertRgba(color: Rgba): void {
  if (!Array.isArray(color) || color.length !== 4) {
    throw new Error(`Paint.color: expected [r, g, b, a], got ${JSON.stringify(color)}`);
  }
  for (const c of color) assertFinite(c, "color", 0, 1);
}

function assertFinite(value: number, name: string, min?: number, max?: number): void {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error(`Paint.${name}: expected a finite number, got ${value}`);
  }
  if (min !== undefined && value < min) throw new Error(`Paint.${name}: ${value} < ${min}`);
  if (max !== undefined && value > max) throw new Error(`Paint.${name}: ${value} > ${max}`);
}

function assertEnum(e: Record<string, unknown>, value: number, name: string): void {
  if (!(value in e) || typeof (e as Record<number, unknown>)[value] !== "string") {
    throw new Error(`Paint.${name}: unknown value ${value}`);
  }
}
