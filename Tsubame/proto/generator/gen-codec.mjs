import { writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import {
  loadProtocolSpec,
  tagToPatchKey,
  toCamelCase,
} from '@hayate/protocol-spec/load';
import { classify, styleEncoderLines } from './value-type.mjs';

const outDir = join(dirname(fileURLToPath(import.meta.url)), '../generated');
const outPath = join(outDir, 'codec.ts');

function enumTsKey(name) {
  return name.replace(/_/g, '-');
}

function appendOpName(opName) {
  const overrides = {
    APPEND_CHILD: 'appendChild',
    INSERT_BEFORE: 'insertBefore',
  };
  if (overrides[opName]) return overrides[opName];
  const pascal = opName
    .split('_')
    .map((w) => w.charAt(0) + w.slice(1).toLowerCase())
    .join('');
  return `append${pascal}`;
}

function generateParsers() {
  return `
export type HayateDimensionUnit = 'px' | 'percent' | 'auto' | 'fr';

export interface HayateDimensionRecord {
  value: number;
  unit: HayateDimensionUnit;
}

export interface HayateColorRecord {
  r: number;
  g: number;
  b: number;
  a: number;
}

export function finiteNumber(key: string, value: unknown): number {
  const numeric = Number(value);
  if (!Number.isFinite(numeric)) {
    throw new Error(\`HayateRenderer: invalid numeric value for "\${key}"\`);
  }
  return numeric;
}

export function finiteInteger(key: string, value: unknown): number {
  const numeric = finiteNumber(key, value);
  if (!Number.isInteger(numeric)) {
    throw new Error(\`HayateRenderer: "\${key}" must be an integer\`);
  }
  return numeric;
}

export function parseDimension(value: import('@tsubame/renderer-protocol').HayateDimension): HayateDimensionRecord {
  if (typeof value === 'number') {
    return { value, unit: 'px' };
  }

  const trimmed = value.trim().toLowerCase();
  if (trimmed === 'auto') {
    return { value: 0, unit: 'auto' };
  }

  const match = trimmed.match(/^(-?(?:\\d+|\\d*\\.\\d+))(px|%|fr)?$/);
  if (match === null) {
    throw new Error(\`HayateRenderer: unsupported dimension "\${value}"\`);
  }

  const numeric = Number(match[1]);
  if (!Number.isFinite(numeric)) {
    throw new Error(\`HayateRenderer: invalid dimension "\${value}"\`);
  }

  const unit = match[2] ?? 'px';
  if (unit === '%') return { value: numeric, unit: 'percent' };
  if (unit === 'fr') return { value: numeric, unit: 'fr' };
  return { value: numeric, unit: 'px' };
}

export function parseColor(input: string): HayateColorRecord {
  const s = input.trim().toLowerCase();
  if (s.startsWith('#')) {
    const hex = s.slice(1);
    const read1 = (i: number): number => parseInt(hex[i]! + hex[i]!, 16) / 255;
    const read2 = (i: number): number => parseInt(hex.slice(i, i + 2), 16) / 255;
    if (hex.length === 3) return { r: read1(0), g: read1(1), b: read1(2), a: 1 };
    if (hex.length === 4) return { r: read1(0), g: read1(1), b: read1(2), a: read1(3) };
    if (hex.length === 6) return { r: read2(0), g: read2(2), b: read2(4), a: 1 };
    if (hex.length === 8) return { r: read2(0), g: read2(2), b: read2(4), a: read2(6) };
  }

  const rgb = s.match(/^rgba?\\((.*)\\)$/);
  if (rgb !== null) {
    const normalized = rgb[1]!.replace(/\\s*\\/\\s*/, ',').replace(/\\s+/g, ',');
    const parts = normalized.split(',').filter(Boolean);
    if (parts.length >= 3) {
      return {
        r: parseColorChannel(parts[0]!),
        g: parseColorChannel(parts[1]!),
        b: parseColorChannel(parts[2]!),
        a: parts[3] === undefined ? 1 : parseAlpha(parts[3]),
      };
    }
  }

  if (s === 'transparent') {
    return { r: 0, g: 0, b: 0, a: 0 };
  }

  throw new Error(\`HayateRenderer: unsupported color "\${input}"\`);
}

function parseColorChannel(raw: string): number {
  const value = raw.trim();
  if (value.endsWith('%')) return clamp01(parseFloat(value) / 100);
  return clamp01(parseFloat(value) / 255);
}

function parseAlpha(raw: string): number {
  const value = raw.trim();
  if (value.endsWith('%')) return clamp01(parseFloat(value) / 100);
  return clamp01(parseFloat(value));
}

function clamp01(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.min(1, Math.max(0, value));
}

/**
 * grid-placement の1スロット（start または end）を [種別タグ, 整数] の2 wire
 * スロットへ符号化する。\`auto\`/undefined は \`[0, 0]\`、line(整数) は \`[1, n]\`、
 * span は \`{ span: n }\` → \`[2, n]\`。
 */
export function encodeGridLine(out: number[], key: string, line: unknown): void {
  if (line === undefined || line === null || line === 'auto') {
    out.push(0, 0);
    return;
  }
  if (typeof line === 'number') {
    out.push(1, finiteInteger(key, line));
    return;
  }
  if (typeof line === 'object' && 'span' in (line as Record<string, unknown>)) {
    out.push(2, finiteInteger(\`\${key}.span\`, (line as { span: unknown }).span));
    return;
  }
  throw new Error(\`HayateRenderer: unsupported grid placement for "\${key}"\`);
}
`.trim();
}

function generateEnumCodeMaps(proto) {
  const lines = [];
  const enumNames = {
    display: 'DISPLAY',
    flex_direction: 'FLEX_DIRECTION',
    flex_wrap: 'FLEX_WRAP',
    align_items: 'ALIGN_ITEMS',
    align_self: 'ALIGN_SELF',
    align_content: 'ALIGN_CONTENT',
    justify_content: 'JUSTIFY_CONTENT',
    font_style: 'FONT_STYLE',
    text_decoration: 'TEXT_DECORATION',
    border_style: 'BORDER_STYLE',
    cursor: 'CURSOR',
    overflow: 'OVERFLOW',
    text_overflow: 'TEXT_OVERFLOW',
    position: 'POSITION',
    transition_timing: 'TRANSITION_TIMING',
    box_sizing: 'BOX_SIZING',
    grid_auto_flow: 'GRID_AUTO_FLOW',
    justify_items: 'JUSTIFY_ITEMS',
    justify_self: 'JUSTIFY_SELF',
  };
  for (const [specName, constName] of Object.entries(enumNames)) {
    const en = (proto.enums ?? []).find((e) => e.name === specName);
    if (!en) continue;
    const mapName = `${constName}_CODE`;
    lines.push(`const ${mapName}: Record<string, number> = {`);
    for (const v of en.values ?? []) {
      lines.push(`  '${enumTsKey(v.name)}': ${constName}.${toCamelCase(v.name)},`);
    }
    lines.push('};');
    lines.push('');
  }
  return lines.join('\n');
}

function generateStyleEncoders(proto) {
  const lines = [];
  for (const tag of proto.style_tags ?? []) {
    const patchKey = tagToPatchKey(tag.name);
    lines.push(...styleEncoderLines(classify(tag), tag.name, patchKey));
    lines.push('');
  }
  return lines.join('\n');
}

function generateAppendOps(proto) {
  const lines = [];
  for (const op of proto.opcodes ?? []) {
    const fnName = appendOpName(op.name);
    const params = (op.params ?? []).map((p) => {
      const tsName = toCamelCase(p.name);
      const count = p.count ?? 0;
      return count > 1 ? `${tsName}: number[]` : `${tsName}: number`;
    });
    const sig = params.length > 0 ? `buf: number[], ${params.join(', ')}` : 'buf: number[]';
    lines.push(`export function ${fnName}(${sig}): void {`);
    lines.push(`  buf.push(OP.${op.name});`);
    for (const p of op.params ?? []) {
      const tsName = toCamelCase(p.name);
      const count = p.count ?? 0;
      if (count > 1) {
        lines.push(`  for (const slot of ${tsName}) buf.push(slot);`);
      } else {
        lines.push(`  buf.push(${tsName});`);
      }
    }
    lines.push('}');
    lines.push('');
  }
  return lines.join('\n');
}

function drawAppendOpName(opName) {
  const pascal = opName
    .split('_')
    .map((w) => w.charAt(0) + w.slice(1).toLowerCase())
    .join('');
  return `appendDraw${pascal}`;
}

// draw display list（`draws` チャネル・ADR-0141/0142）の表駆動 encoder。
// path verb は固定スロット、draw command（FILL）は paint_len プレフィックス付きの
// tagged paint packet を後続に持つ（Rust decode `decode_draw_list` と対）。
function generateDrawAppendOps(proto) {
  const lines = [];

  lines.push('export interface DrawPaint {');
  for (const field of proto.draw_paint_fields ?? []) {
    const key = tagToPatchKey(field.name);
    const slots = (field.params ?? []).reduce((n, p) => n + (p.count > 1 ? p.count : 1), 0);
    const tsType =
      (field.params ?? []).length > 1
        ? `readonly [${Array(slots).fill('number').join(', ')}]`
        : 'number';
    lines.push(`  readonly ${key}?: ${tsType};`);
  }
  lines.push('}');
  lines.push('');

  for (const op of proto.draw_ops ?? []) {
    const fnName = drawAppendOpName(op.name);
    if (op.drawRole === 'path-verb') {
      const params = (op.params ?? []).map((p) => `${toCamelCase(p.name)}: number`);
      const sig = params.length > 0 ? `draws: number[], ${params.join(', ')}` : 'draws: number[]';
      const pushes = (op.params ?? []).map((p) => toCamelCase(p.name));
      lines.push(`export function ${fnName}(${sig}): void {`);
      lines.push(`  draws.push(${[`DRAW_OP.${op.name}`, ...pushes].join(', ')});`);
      lines.push('}');
      lines.push('');
      continue;
    }
    if (op.name !== 'FILL') {
      throw new Error(`draw_ops.${op.name}: unhandled draw-command encoder (add an arm)`);
    }
    lines.push(`export function ${fnName}(draws: number[], paint: DrawPaint): void {`);
    lines.push(`  draws.push(DRAW_OP.${op.name});`);
    lines.push('  const lenIndex = draws.length;');
    lines.push('  draws.push(0);');
    for (const field of proto.draw_paint_fields ?? []) {
      const key = tagToPatchKey(field.name);
      lines.push(`  if (paint.${key} !== undefined) {`);
      if ((field.params ?? []).length > 1) {
        lines.push(`    draws.push(DRAW_PAINT_FIELD.${field.name}, ...paint.${key});`);
      } else {
        lines.push(`    draws.push(DRAW_PAINT_FIELD.${field.name}, paint.${key});`);
      }
      lines.push('  }');
    }
    lines.push('  draws[lenIndex] = draws.length - lenIndex - 1;');
    lines.push('}');
    lines.push('');
  }
  return lines.join('\n');
}

export function generateCodec() {
  const proto = loadProtocolSpec();

  const styleEncoderMap = (proto.style_tags ?? [])
    .map((tag) => {
      const patchKey = tagToPatchKey(tag.name);
      return `  ${patchKey}: encode_${patchKey},`;
    })
    .join('\n');

  const inheritedUnset = (proto.unset_kinds ?? [])
    .map((uk) => {
      const patchKey = tagToPatchKey(uk.name);
      return `  ${patchKey}: UNSET_KIND.${toCamelCase(uk.name)},`;
    })
    .join('\n');

  const lines = [
    '// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと',
    '// 生成元: @hayate/protocol-spec',
    '',
    "import type { StylePatch } from '@tsubame/renderer-protocol';",
    "import { OP, DRAW_OP, DRAW_PAINT_FIELD, TAG, UNSET_KIND, UNIT_CODE, DISPLAY, FLEX_DIRECTION, FLEX_WRAP, ALIGN_ITEMS, ALIGN_SELF, ALIGN_CONTENT, JUSTIFY_CONTENT, FONT_STYLE, TEXT_DECORATION, BORDER_STYLE, CURSOR, OVERFLOW, TEXT_OVERFLOW, POSITION, TRANSITION_TIMING, BOX_SIZING, GRID_AUTO_FLOW, JUSTIFY_ITEMS, JUSTIFY_SELF } from './protocol.js';",
    '',
    'export { TAG, UNSET_KIND } from \'./protocol.js\';',
    '',
    generateParsers(),
    '',
    generateEnumCodeMaps(proto),
    generateStyleEncoders(proto),
    'const STYLE_ENCODERS = {',
    styleEncoderMap,
    '} as Partial<Record<keyof StylePatch, (out: number[], value: unknown) => void>>;',
    '',
    'const INHERITED_UNSET: Partial<Record<string, number>> = {',
    inheritedUnset,
    '};',
    '',
    '/** StylePatch の SET 部分を style-packet の TAG ワイヤースロットへエンコードする。 */',
    'export function encodeStylePatch(patch: StylePatch, out: number[]): void {',
    '  for (const key in patch) {',
    '    const k = key as keyof StylePatch;',
    '    const value = patch[k];',
    '    if (value === undefined || value === null) continue;',
    '    const encoder = STYLE_ENCODERS[k];',
    '    if (encoder === undefined) {',
    '      throw new Error(`HayateRenderer: unsupported style property "${String(k)}"`);',
    '    }',
    '    encoder(out, value);',
    '  }',
    '}',
    '',
    '/** StylePatch 内の継承プロパティの null リセットを OP_UNSET_STYLE の種別コードへ対応付ける。 */',
    'export function unsetKindsOf(patch: StylePatch): number[] {',
    '  const kinds: number[] = [];',
    '  for (const key in patch) {',
    '    const k = key as keyof StylePatch;',
    '    if (patch[k] !== null) continue;',
    '    const code = INHERITED_UNSET[k as string];',
    '    if (code === undefined) {',
    '      throw new Error(`HayateRenderer: cannot reset non-inheritable property "${String(k)}"`);',
    '    }',
    '    kinds.push(code);',
    '  }',
    '  return kinds;',
    '}',
    '',
    generateAppendOps(proto),
    generateDrawAppendOps(proto),
  ];

  mkdirSync(outDir, { recursive: true });
  writeFileSync(outPath, lines.join('\n'), 'utf8');
}
