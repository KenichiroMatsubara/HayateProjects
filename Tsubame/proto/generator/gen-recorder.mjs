import { writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { loadProtocolSpec, toCamelCase } from '@torimi/hayate-protocol-spec/load';

const outDir = join(dirname(fileURLToPath(import.meta.url)), '../generated');
const outPath = join(outDir, 'recorder.ts');

/** draw op 名 → appendDraw<Pascal>（gen-codec と同規則）。 */
function appendDrawName(opName) {
  const pascal = opName
    .split('_')
    .map((w) => w.charAt(0) + w.slice(1).toLowerCase())
    .join('');
  return `appendDraw${pascal}`;
}

/** Flutter 語彙のメソッド名（未登録の新 op は camelCase にフォールバック — 手書き不要）。 */
const PATH_METHODS = {
  MOVE_TO: 'moveTo',
  LINE_TO: 'lineTo',
  QUADRATIC_TO: 'quadraticBezierTo',
  CUBIC_TO: 'cubicTo',
  ARC_TO: 'arcTo',
  RECT: 'addRect',
  RRECT: 'addRRect',
  OVAL: 'addOval',
  CIRCLE: 'addCircle',
  CLOSE: 'close',
};
// gen-draw-canvas（painter 向け DrawCanvas インターフェース）と共有し、
// recorder 実装と型サーフェスのメソッド名が機械的に一致するようにする（#730）。
export const CANVAS_METHODS = {
  SAVE: 'save',
  RESTORE: 'restore',
  TRANSLATE: 'translate',
  ROTATE: 'rotate',
  SCALE: 'scale',
  TRANSFORM: 'transform',
  CLIP_RECT: 'clipRect',
};

// drawPath / clipPath / fill / stroke は Path・Paint を取る意味的な特別扱い。
export const SPECIAL_COMMANDS = new Set(['FILL', 'STROKE', 'CLIP_PATH']);

export function generateRecorder() {
  const proto = loadProtocolSpec();
  const ops = proto.draw_ops ?? [];
  const pathVerbs = ops.filter((op) => op.drawRole === 'path-verb');
  const structuralCommands = ops.filter(
    (op) => op.drawRole === 'draw-command' && !SPECIAL_COMMANDS.has(op.name),
  );

  // import する appendDraw* を集める。
  const imports = new Set(['appendDrawFill', 'appendDrawStroke', 'appendDrawClipPath']);
  for (const op of [...pathVerbs, ...structuralCommands]) imports.add(appendDrawName(op.name));

  const lines = [];
  lines.push('// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと');
  lines.push('// 生成元: @torimi/hayate-protocol-spec（draw_ops / draw_paint_fields）。');
  lines.push('// アプリ作者が painter 内で使う Flutter 流ステートレス記録 API（#729 / ADR-0143）。');
  lines.push('// Path / Paint は第一級オブジェクト、canvas.drawPath(path, paint) で呼び出しごとに');
  lines.push('// paint を明示する。中身は spec の op 表から表駆動で生成した薄い encoder で、');
  lines.push('// draws バッファ（f32）へ書き出すだけ（手書きの意味論を持たない）。');
  lines.push('');
  lines.push('/* eslint-disable */');
  lines.push(`import {\n  ${[...imports].sort().join(',\n  ')},\n  type DrawPaint,\n} from './codec.js';`);
  lines.push("import type { DrawCanvas } from '@torimi/tsubame-renderer-protocol';");
  lines.push('');
  lines.push('/** Flutter PaintingStyle（fill = 塗り、stroke = 輪郭）。 */');
  lines.push('export enum PaintingStyle {\n  fill = 0,\n  stroke = 1,\n}');
  lines.push('');
  lines.push('/** Flutter StrokeCap（Hayate line_cap enum）。 */');
  lines.push('export enum StrokeCap {\n  butt = 0,\n  round = 1,\n  square = 2,\n}');
  lines.push('');
  lines.push('/** Flutter StrokeJoin（Hayate line_join enum）。 */');
  lines.push('export enum StrokeJoin {\n  miter = 0,\n  round = 1,\n  bevel = 2,\n}');
  lines.push('');
  lines.push('/** Flutter PathFillType（Hayate fill_rule enum）。 */');
  lines.push('export enum PathFillType {\n  nonZero = 0,\n  evenOdd = 1,\n}');
  lines.push('');
  lines.push('/** ストレート RGBA（各 0..1）。 */');
  lines.push('export type Rgba = readonly [number, number, number, number];');
  lines.push('');

  // ── Paint ────────────────────────────────────────────────────────────────
  lines.push('/**');
  lines.push(' * 描画スタイル。fill / stroke を `style` で切り替える。閉じた語彙（Renderer');
  lines.push(' * Protocol）: `toDrawPaint()` は不正な enum 値・範囲外をエラーにする。');
  lines.push(' */');
  lines.push('export class Paint {');
  lines.push('  color: Rgba = [0, 0, 0, 1];');
  lines.push('  style: PaintingStyle = PaintingStyle.fill;');
  lines.push('  fillType: PathFillType = PathFillType.nonZero;');
  lines.push('  strokeWidth = 1;');
  lines.push('  strokeCap: StrokeCap = StrokeCap.butt;');
  lines.push('  strokeJoin: StrokeJoin = StrokeJoin.miter;');
  lines.push('  strokeMiterLimit = 4;');
  lines.push('  dash: readonly number[] = [];');
  lines.push('  dashOffset = 0;');
  lines.push('');
  lines.push('  /** 現在のフィールドを wire の DrawPaint パケットへ解決する（不正値はエラー）。 */');
  lines.push('  toDrawPaint(): DrawPaint {');
  lines.push('    assertRgba(this.color);');
  lines.push('    assertEnum(PathFillType, this.fillType, "fillType");');
  lines.push('    assertEnum(PaintingStyle, this.style, "style");');
  lines.push('    const paint: {\n      -readonly [K in keyof DrawPaint]: DrawPaint[K];\n    } = { color: this.color };');
  lines.push('    if (this.fillType !== PathFillType.nonZero) paint.fillRule = this.fillType;');
  lines.push('    if (this.style === PaintingStyle.stroke) {');
  lines.push('      assertFinite(this.strokeWidth, "strokeWidth", 0);');
  lines.push('      assertEnum(StrokeCap, this.strokeCap, "strokeCap");');
  lines.push('      assertEnum(StrokeJoin, this.strokeJoin, "strokeJoin");');
  lines.push('      assertFinite(this.strokeMiterLimit, "strokeMiterLimit", 0);');
  lines.push('      paint.strokeWidth = this.strokeWidth;');
  lines.push('      paint.cap = this.strokeCap;');
  lines.push('      paint.join = this.strokeJoin;');
  lines.push('      paint.miterLimit = this.strokeMiterLimit;');
  lines.push('      if (this.dash.length > 0) {');
  lines.push('        for (const d of this.dash) assertFinite(d, "dash", 0);');
  lines.push('        paint.dash = this.dash;');
  lines.push('        assertFinite(this.dashOffset, "dashOffset");');
  lines.push('        paint.dashOffset = this.dashOffset;');
  lines.push('      }');
  lines.push('    }');
  lines.push('    return paint;');
  lines.push('  }');
  lines.push('}');
  lines.push('');

  // ── Path ─────────────────────────────────────────────────────────────────
  lines.push('/**');
  lines.push(' * 記録済みパス。フレーム間・要素間で再利用できる不変の op 列として保持し、');
  lines.push(' * `canvas.drawPath` / `canvas.clipPath` で何度でも再生できる。メソッドは');
  lines.push(' * draw_ops の path-verb から生成される（op 追加時に手書き修正は不要）。');
  lines.push(' */');
  lines.push('export class Path {');
  lines.push('  private readonly ops: number[] = [];');
  lines.push('');
  for (const op of pathVerbs) {
    const name = PATH_METHODS[op.name] ?? toCamelCase(op.name);
    const params = (op.params ?? []).map((p) => `${toCamelCase(p.name)}: number`);
    const args = (op.params ?? []).map((p) => toCamelCase(p.name));
    const sig = params.join(', ');
    lines.push(`  ${name}(${sig}): this {`);
    lines.push(`    ${appendDrawName(op.name)}(${['this.ops', ...args].join(', ')});`);
    lines.push('    return this;');
    lines.push('  }');
    lines.push('');
  }
  lines.push('  /** 記録済み op 列（再生用の読み取り専用ビュー。Path は不変）。 */');
  lines.push('  record(): readonly number[] {');
  lines.push('    return this.ops;');
  lines.push('  }');
  lines.push('}');
  lines.push('');

  // ── Canvas ───────────────────────────────────────────────────────────────
  lines.push('/**');
  lines.push(' * draws バッファへの記録面。Flutter/Skia 流ステートレス設計: canvas 自体の');
  lines.push(' * 状態は save/restore の変換・クリップスタックのみ。座標操作・クリップ矩形の');
  lines.push(' * メソッドは draw_ops の構造 command から生成される。painter へは Renderer');
  lines.push(' * Protocol の `DrawCanvas`（同じ op 表から生成・#730）として渡り、implements で');
  lines.push(' * 型サーフェスとの drift をコンパイル時に検出する。');
  lines.push(' */');
  lines.push('export class Canvas implements DrawCanvas {');
  lines.push('  private readonly buf: number[] = [];');
  lines.push('');
  lines.push('  /** `path` を `paint` で塗る / 輪郭描画する（paint.style で分岐）。 */');
  lines.push('  drawPath(path: Path, paint: Paint): this {');
  lines.push('    for (const v of path.record()) this.buf.push(v);');
  lines.push('    if (paint.style === PaintingStyle.stroke) {');
  lines.push('      appendDrawStroke(this.buf, paint.toDrawPaint());');
  lines.push('    } else {');
  lines.push('      appendDrawFill(this.buf, paint.toDrawPaint());');
  lines.push('    }');
  lines.push('    return this;');
  lines.push('  }');
  lines.push('');
  lines.push('  /** 以降の描画を `path` で切り抜く（対応する restore で解除）。 */');
  lines.push('  clipPath(path: Path): this {');
  lines.push('    for (const v of path.record()) this.buf.push(v);');
  lines.push('    appendDrawClipPath(this.buf);');
  lines.push('    return this;');
  lines.push('  }');
  lines.push('');
  for (const op of structuralCommands) {
    const name = CANVAS_METHODS[op.name] ?? toCamelCase(op.name);
    const params = (op.params ?? []).map((p) => `${toCamelCase(p.name)}: number`);
    const args = (op.params ?? []).map((p) => toCamelCase(p.name));
    const sig = params.join(', ');
    lines.push(`  ${name}(${sig}): this {`);
    lines.push(`    ${appendDrawName(op.name)}(${['this.buf', ...args].join(', ')});`);
    lines.push('    return this;');
    lines.push('  }');
    lines.push('');
  }
  lines.push('  /** 記録した display list（draws チャネルへ載せる f32 列）。 */');
  lines.push('  finish(): number[] {');
  lines.push('    return this.buf;');
  lines.push('  }');
  lines.push('}');
  lines.push('');

  // ── 検証ヘルパ（閉じた語彙） ───────────────────────────────────────────────
  lines.push('function assertRgba(color: Rgba): void {');
  lines.push('  if (!Array.isArray(color) || color.length !== 4) {');
  lines.push('    throw new Error(`Paint.color: expected [r, g, b, a], got ${JSON.stringify(color)}`);');
  lines.push('  }');
  lines.push('  for (const c of color) assertFinite(c, "color", 0, 1);');
  lines.push('}');
  lines.push('');
  lines.push('function assertFinite(value: number, name: string, min?: number, max?: number): void {');
  lines.push('  if (typeof value !== "number" || !Number.isFinite(value)) {');
  lines.push('    throw new Error(`Paint.${name}: expected a finite number, got ${value}`);');
  lines.push('  }');
  lines.push('  if (min !== undefined && value < min) throw new Error(`Paint.${name}: ${value} < ${min}`);');
  lines.push('  if (max !== undefined && value > max) throw new Error(`Paint.${name}: ${value} > ${max}`);');
  lines.push('}');
  lines.push('');
  lines.push('function assertEnum(e: Record<string, unknown>, value: number, name: string): void {');
  lines.push('  if (!(value in e) || typeof (e as Record<number, unknown>)[value] !== "string") {');
  lines.push('    throw new Error(`Paint.${name}: unknown value ${value}`);');
  lines.push('  }');
  lines.push('}');
  lines.push('');

  mkdirSync(outDir, { recursive: true });
  writeFileSync(outPath, lines.join('\n'), 'utf8');
}
