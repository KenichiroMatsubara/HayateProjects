import { writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { loadProtocolSpec, tagToPatchKey } from '@torimi/hayate-protocol-spec/load';
import { classify, tsType } from './value-type.mjs';

const outDir = join(
  dirname(fileURLToPath(import.meta.url)),
  '../../packages/renderer-protocol/src/generated',
);
const outPath = join(outDir, 'style-types.ts');

function enumTsKey(name) {
  return name.replace(/_/g, '-');
}

const ENUM_TYPES = [
  { specName: 'display', typeName: 'Display' },
  { specName: 'flex_direction', typeName: 'FlexDirection' },
  { specName: 'flex_wrap', typeName: 'FlexWrap' },
  { specName: 'align_items', typeName: 'AlignItems' },
  { specName: 'align_self', typeName: 'AlignSelf' },
  { specName: 'align_content', typeName: 'AlignContent' },
  { specName: 'justify_content', typeName: 'JustifyContent' },
  { specName: 'font_style', typeName: 'FontStyle' },
  { specName: 'text_decoration', typeName: 'TextDecoration' },
  { specName: 'border_style', typeName: 'BorderStyle' },
  { specName: 'cursor', typeName: 'Cursor' },
  { specName: 'overflow', typeName: 'Overflow' },
  { specName: 'text_overflow', typeName: 'TextOverflow' },
  { specName: 'position', typeName: 'Position' },
  { specName: 'transition_timing', typeName: 'TransitionTiming' },
  { specName: 'box_sizing', typeName: 'BoxSizing' },
  { specName: 'grid_auto_flow', typeName: 'GridAutoFlow' },
  { specName: 'justify_items', typeName: 'JustifyItems' },
  { specName: 'justify_self', typeName: 'JustifySelf' },
];

function generateEnumTypes(proto) {
  const lines = [];
  for (const { specName, typeName } of ENUM_TYPES) {
    const en = (proto.enums ?? []).find((e) => e.name === specName);
    if (!en) {
      throw new Error(`enums: missing "${specName}"`);
    }
    const variants = (en.values ?? []).map((v) => `'${enumTsKey(v.name)}'`).join(' | ');
    lines.push(`export type ${typeName} = ${variants};`);
  }
  return lines.join('\n');
}

function generateHayateStyle(proto) {
  const lines = ['export interface HayateStyle {'];
  for (const tag of proto.style_tags ?? []) {
    const patchKey = tagToPatchKey(tag.name);
    lines.push(`  ${patchKey}: ${tsType(classify(tag))};`);
  }
  lines.push('}');
  return lines.join('\n');
}

export function generateStyleTypes() {
  const proto = loadProtocolSpec();

  const lines = [
    '// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと',
    '// 生成元: @torimi/hayate-protocol-spec',
    '',
    "import type { HayateDimension, HayateGridPlacement, HayateShadow } from '../style-primitives.js';",
    '',
    generateEnumTypes(proto),
    '',
    generateHayateStyle(proto),
    '',
    '/**',
    ' * `IRenderer.setStyle` のパッチ意味論。',
    ' *',
    ' * - 存在するプロパティは以前の値を上書きする。',
    ' * - 存在しないプロパティは以前の値を保持する。',
    ' * - `null` は、対象レンダラーがリセットに対応している場合にプロパティをリセットする。',
    ' */',
    'export type StylePatch = {',
    '  [K in keyof HayateStyle]?: HayateStyle[K] | null;',
    '};',
    '',
  ];

  mkdirSync(outDir, { recursive: true });
  writeFileSync(outPath, lines.join('\n'), 'utf8');
}
