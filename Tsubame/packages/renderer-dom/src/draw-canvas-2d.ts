import type {
  DrawCanvas,
  DrawPaintPacket,
  DrawPaintSource,
  DrawRecordedPath,
} from '@torimi/tsubame-renderer-protocol';
import { DRAW_OP } from '@torimi/tsubame-protocol-generated/protocol';

/**
 * 2D replay が使う `CanvasRenderingContext2D` の最小サブセット（#731 /
 * Tsubame ADR-0014）。実ブラウザの 2D コンテキストが構造的に満たし、テストは
 * 呼び出し列を記録するモックで置き換える（実ピクセルは見ない）。
 */
export interface Draw2DContext {
  fillStyle: string | CanvasGradient | CanvasPattern;
  strokeStyle: string | CanvasGradient | CanvasPattern;
  lineWidth: number;
  lineCap: CanvasLineCap;
  lineJoin: CanvasLineJoin;
  miterLimit: number;
  lineDashOffset: number;
  save(): void;
  restore(): void;
  translate(dx: number, dy: number): void;
  rotate(radians: number): void;
  scale(sx: number, sy: number): void;
  transform(a: number, b: number, c: number, d: number, e: number, f: number): void;
  setTransform(a: number, b: number, c: number, d: number, e: number, f: number): void;
  clearRect(x: number, y: number, width: number, height: number): void;
  beginPath(): void;
  moveTo(x: number, y: number): void;
  lineTo(x: number, y: number): void;
  closePath(): void;
  quadraticCurveTo(cx: number, cy: number, x: number, y: number): void;
  bezierCurveTo(c1x: number, c1y: number, c2x: number, c2y: number, x: number, y: number): void;
  arcTo(x1: number, y1: number, x2: number, y2: number, radius: number): void;
  rect(x: number, y: number, width: number, height: number): void;
  roundRect(x: number, y: number, width: number, height: number, radii: ReadonlyArray<{ x: number; y: number }>): void;
  ellipse(cx: number, cy: number, rx: number, ry: number, rotation: number, start: number, end: number): void;
  arc(cx: number, cy: number, radius: number, start: number, end: number): void;
  fill(fillRule: CanvasFillRule): void;
  stroke(): void;
  clip(): void;
  setLineDash(segments: readonly number[]): void;
}

// spec enum（enums.json の line_cap / line_join / fill_rule）→ 2D コンテキスト語彙。
const CAP_STYLES: readonly CanvasLineCap[] = ['butt', 'round', 'square'];
const JOIN_STYLES: readonly CanvasLineJoin[] = ['miter', 'round', 'bevel'];
const FILL_RULES: readonly CanvasFillRule[] = ['nonzero', 'evenodd'];

// DrawPaintPacket は指定フィールドのみ運ぶ（未指定 = spec 既定）。replay は
// ステートレスに毎 draw 全プロパティを設定するので、既定値をここで解決する。
const DEFAULT_COLOR: readonly [number, number, number, number] = [0, 0, 0, 1];
const DEFAULT_STROKE_WIDTH = 1;
const DEFAULT_MITER_LIMIT = 4;
const NO_DASH: readonly number[] = [];

const TWO_PI = Math.PI * 2;

/** ストレート RGBA（0..1）→ CSS の `rgba(r, g, b, a)`（rgb は 0..255）。 */
function rgbaString(color: readonly [number, number, number, number]): string {
  const to255 = (c: number): number => Math.round(c * 255);
  return `rgba(${to255(color[0])}, ${to255(color[1])}, ${to255(color[2])}, ${color[3]})`;
}

/** 記録済み path op 列（draw_ops の path-verb）を 2D コンテキストの path verb へ写す。 */
function emitPath(ctx: Draw2DContext, path: DrawRecordedPath): void {
  const ops = path.record();
  ctx.beginPath();
  for (let i = 0; i < ops.length; ) {
    const op = ops[i]!;
    switch (op) {
      case DRAW_OP.MOVE_TO:
        ctx.moveTo(ops[i + 1]!, ops[i + 2]!);
        i += 3;
        break;
      case DRAW_OP.LINE_TO:
        ctx.lineTo(ops[i + 1]!, ops[i + 2]!);
        i += 3;
        break;
      case DRAW_OP.CLOSE:
        ctx.closePath();
        i += 1;
        break;
      case DRAW_OP.QUADRATIC_TO:
        ctx.quadraticCurveTo(ops[i + 1]!, ops[i + 2]!, ops[i + 3]!, ops[i + 4]!);
        i += 5;
        break;
      case DRAW_OP.CUBIC_TO:
        ctx.bezierCurveTo(ops[i + 1]!, ops[i + 2]!, ops[i + 3]!, ops[i + 4]!, ops[i + 5]!, ops[i + 6]!);
        i += 7;
        break;
      case DRAW_OP.ARC_TO:
        ctx.arcTo(ops[i + 1]!, ops[i + 2]!, ops[i + 3]!, ops[i + 4]!, ops[i + 5]!);
        i += 6;
        break;
      case DRAW_OP.RECT:
        ctx.rect(ops[i + 1]!, ops[i + 2]!, ops[i + 3]!, ops[i + 4]!);
        i += 5;
        break;
      case DRAW_OP.RRECT:
        ctx.roundRect(ops[i + 1]!, ops[i + 2]!, ops[i + 3]!, ops[i + 4]!, [
          { x: ops[i + 5]!, y: ops[i + 6]! },
        ]);
        i += 7;
        break;
      case DRAW_OP.OVAL: {
        const [x, y, width, height] = [ops[i + 1]!, ops[i + 2]!, ops[i + 3]!, ops[i + 4]!];
        const rx = width / 2;
        const ry = height / 2;
        const cx = x + rx;
        const cy = y + ry;
        // 閉じた独立 subpath として追加する（recorder / Rust decode と同じ意味論）。
        // 角度 0 の開始点へ moveTo してから全周を描くと、直前 subpath からの接続線が
        // 退化して現れない。
        ctx.moveTo(cx + rx, cy);
        ctx.ellipse(cx, cy, rx, ry, 0, 0, TWO_PI);
        ctx.closePath();
        i += 5;
        break;
      }
      case DRAW_OP.CIRCLE: {
        const [cx, cy, radius] = [ops[i + 1]!, ops[i + 2]!, ops[i + 3]!];
        ctx.moveTo(cx + radius, cy);
        ctx.arc(cx, cy, radius, 0, TWO_PI);
        ctx.closePath();
        i += 4;
        break;
      }
      default:
        // Renderer Protocol の閉じた語彙: 未知 op は黙って落とさない。
        throw new Error(`Canvas2DReplay: unknown path op ${op}`);
    }
  }
}

const PAINTING_STYLE_STROKE = 1;

/**
 * painter の記録面の 2D バックエンド（Tsubame ADR-0014）。`DrawCanvas` 契約を
 * recorder（wire 記録）と共有し、同じ painter が Hayate Renderer と同じ絵を
 * `<canvas>` 2D に出す。wire は通らない: painter の呼び出しを 2D コンテキストの
 * 呼び出しへその場で写す。canvas 自体の状態は save/restore の変換・クリップ
 * スタックのみ（ステートレス: paint は draw 呼び出しごとに全指定）。
 */
export class Canvas2DReplay implements DrawCanvas {
  constructor(private readonly ctx: Draw2DContext) {}

  drawPath(path: DrawRecordedPath, paint: DrawPaintSource): this {
    const packet = paint.toDrawPaint();
    emitPath(this.ctx, path);
    if (paint.style === PAINTING_STYLE_STROKE) {
      this.applyStroke(packet);
      this.ctx.stroke();
    } else {
      this.ctx.fillStyle = rgbaString(packet.color ?? DEFAULT_COLOR);
      this.ctx.fill(FILL_RULES[packet.fillRule ?? 0]!);
    }
    return this;
  }

  clipPath(path: DrawRecordedPath): this {
    emitPath(this.ctx, path);
    this.ctx.clip();
    return this;
  }

  save(): this {
    this.ctx.save();
    return this;
  }

  restore(): this {
    this.ctx.restore();
    return this;
  }

  translate(dx: number, dy: number): this {
    this.ctx.translate(dx, dy);
    return this;
  }

  rotate(radians: number): this {
    this.ctx.rotate(radians);
    return this;
  }

  scale(sx: number, sy: number): this {
    this.ctx.scale(sx, sy);
    return this;
  }

  transform(a: number, b: number, c: number, d: number, e: number, f: number): this {
    this.ctx.transform(a, b, c, d, e, f);
    return this;
  }

  clipRect(x: number, y: number, width: number, height: number): this {
    this.ctx.beginPath();
    this.ctx.rect(x, y, width, height);
    this.ctx.clip();
    return this;
  }

  /** stroke 系プロパティを毎回全設定する（前の draw の値を漏らさない）。 */
  private applyStroke(packet: DrawPaintPacket): void {
    this.ctx.strokeStyle = rgbaString(packet.color ?? DEFAULT_COLOR);
    this.ctx.lineWidth = packet.strokeWidth ?? DEFAULT_STROKE_WIDTH;
    this.ctx.lineCap = CAP_STYLES[packet.cap ?? 0]!;
    this.ctx.lineJoin = JOIN_STYLES[packet.join ?? 0]!;
    this.ctx.miterLimit = packet.miterLimit ?? DEFAULT_MITER_LIMIT;
    this.ctx.setLineDash(packet.dash ?? NO_DASH);
    this.ctx.lineDashOffset = packet.dashOffset ?? 0;
  }
}
