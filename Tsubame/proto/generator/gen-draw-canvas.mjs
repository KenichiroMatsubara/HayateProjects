import { writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import {
  loadProtocolSpec,
  tagToPatchKey,
  toCamelCase,
} from '@torimi/hayate-protocol-spec/load';
import { CANVAS_METHODS, SPECIAL_COMMANDS } from './gen-recorder.mjs';

const outDir = join(
  dirname(fileURLToPath(import.meta.url)),
  '../../packages/renderer-protocol/src/generated',
);
const outPath = join(outDir, 'draw-canvas.ts');

export function generateDrawCanvas() {
  const proto = loadProtocolSpec();
  const ops = proto.draw_ops ?? [];
  const structuralCommands = ops.filter(
    (op) => op.drawRole === 'draw-command' && !SPECIAL_COMMANDS.has(op.name),
  );

  const lines = [];
  lines.push('// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと');
  lines.push('// 生成元: @torimi/hayate-protocol-spec（draw_ops / draw_paint_fields）');
  lines.push('//');
  lines.push('// painter（`draw` property の値）が受け取る記録面の構造的インターフェース');
  lines.push('//（#730 / ADR-0141）。実体は Hayate Renderer では wire 記録の recorder Canvas');
  lines.push('//（@torimi/tsubame-protocol-generated/recorder）、DOM Renderer では canvas 2D への');
  lines.push('// replay（Tsubame ADR-0014）。recorder と同じ op 表から生成するため、spec に');
  lines.push('// op が増えても手書き修正なしで painter の型が追随する（Script Adapter 規律）。');
  lines.push('// `finish()` はフレームワーク側の口なので意図的に含めない（painter はバッファに');
  lines.push('// 触れない）。');
  lines.push('');
  lines.push('/** 記録済みパスの最小 surface（recorder `Path` が満たす）。 */');
  lines.push('export interface DrawRecordedPath {');
  lines.push('  /** 記録済み op 列（再生用の読み取り専用ビュー）。 */');
  lines.push('  record(): readonly number[];');
  lines.push('}');
  lines.push('');
  lines.push('/** Paint を wire 解決した tagged パケット（codec `DrawPaint` と同形）。 */');
  lines.push('export interface DrawPaintPacket {');
  for (const field of proto.draw_paint_fields ?? []) {
    const key = tagToPatchKey(field.name);
    const slots = (field.params ?? []).reduce((n, p) => n + (p.count > 1 ? p.count : 1), 0);
    let tsType;
    if (field.variable_length === true) {
      tsType = 'readonly number[]';
    } else if ((field.params ?? []).length > 1) {
      tsType = `readonly [${Array(slots).fill('number').join(', ')}]`;
    } else {
      tsType = 'number';
    }
    lines.push(`  readonly ${key}?: ${tsType};`);
  }
  lines.push('}');
  lines.push('');
  lines.push('/** `drawPath` に渡す Paint の最小 surface（recorder `Paint` が満たす）。 */');
  lines.push('export interface DrawPaintSource {');
  lines.push('  /** PaintingStyle（0 = fill, 1 = stroke）。 */');
  lines.push('  readonly style: number;');
  lines.push('  /** 現在のフィールドを wire パケットへ解決する（不正値はエラー）。 */');
  lines.push('  toDrawPaint(): DrawPaintPacket;');
  lines.push('}');
  lines.push('');
  lines.push('/**');
  lines.push(' * painter の記録面。Flutter/Skia 流ステートレス設計: canvas 自体の状態は');
  lines.push(' * save/restore の変換・クリップスタックのみ。座標はボーダーボックス左上原点・');
  lines.push(' * 論理 px・DPR 不可視（ADR-0141）。');
  lines.push(' */');
  lines.push('export interface DrawCanvas {');
  lines.push('  /** `path` を `paint` で塗る / 輪郭描画する（paint.style で分岐）。 */');
  lines.push('  drawPath(path: DrawRecordedPath, paint: DrawPaintSource): this;');
  lines.push('');
  lines.push('  /** 以降の描画を `path` で切り抜く（対応する restore で解除）。 */');
  lines.push('  clipPath(path: DrawRecordedPath): this;');
  lines.push('');
  for (const op of structuralCommands) {
    const name = CANVAS_METHODS[op.name] ?? toCamelCase(op.name);
    const params = (op.params ?? []).map((p) => `${toCamelCase(p.name)}: number`);
    lines.push(`  ${name}(${params.join(', ')}): this;`);
    lines.push('');
  }
  lines.push('}');
  lines.push('');

  mkdirSync(outDir, { recursive: true });
  writeFileSync(outPath, lines.join('\n'), 'utf8');
}
