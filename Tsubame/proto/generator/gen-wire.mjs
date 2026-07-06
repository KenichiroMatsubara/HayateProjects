import { writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { loadProtocolSpec, toCamelCase } from '@hayate/protocol-spec/load';

const outDir = join(dirname(fileURLToPath(import.meta.url)), '../generated');
const outPath = join(outDir, 'protocol.ts');

export function generateWire() {
  const proto = loadProtocolSpec();

  const lines = [
    '// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと',
    '// 生成元: @hayate/protocol-spec',
    '',
  ];

  // wire の protocol version（バンドル encoder ↔ ホスト decoder の整合トークン）。spec manifest の
  // `version` を唯一の source of truth として焼き出す（#530 / CONTEXT.md「Protocol Version」）。
  lines.push(`export const PROTOCOL_VERSION = ${proto.manifest.version};`);
  lines.push('');

  lines.push('export const OP = {');
  for (const op of proto.opcodes ?? []) {
    lines.push(`  ${op.name}: ${op.value},`);
  }
  lines.push('} as const;');
  lines.push('export type OP = typeof OP;');
  lines.push('');

  // draw display list（`draws` チャネル・ADR-0141/0142）の op / paint field 定数。
  lines.push('export const DRAW_OP = {');
  for (const op of proto.draw_ops ?? []) {
    lines.push(`  ${op.name}: ${op.value},`);
  }
  lines.push('} as const;');
  lines.push('export type DRAW_OP = typeof DRAW_OP;');
  lines.push('');

  lines.push('export const DRAW_PAINT_FIELD = {');
  for (const field of proto.draw_paint_fields ?? []) {
    lines.push(`  ${field.name}: ${field.value},`);
  }
  lines.push('} as const;');
  lines.push('export type DRAW_PAINT_FIELD = typeof DRAW_PAINT_FIELD;');
  lines.push('');

  lines.push('export const TAG = {');
  for (const tag of proto.style_tags ?? []) {
    lines.push(`  ${tag.name}: ${tag.value},`);
  }
  lines.push('} as const;');
  lines.push('export type TAG = typeof TAG;');
  lines.push('');

  lines.push('export const EVENT_KIND = {');
  for (const ev of proto.event_kinds ?? []) {
    lines.push(`  ${ev.name.toUpperCase()}: ${ev.value},`);
  }
  lines.push('} as const;');
  lines.push('export type EVENT_KIND = typeof EVENT_KIND;');
  lines.push('');

  lines.push('export const EVENT_WIRE_ROLE = {');
  for (const ev of proto.event_kinds ?? []) {
    lines.push(`  ${ev.name.toUpperCase()}: '${ev.wireRole}',`);
  }
  lines.push('} as const;');
  lines.push('export type EVENT_WIRE_ROLE = typeof EVENT_WIRE_ROLE;');
  lines.push('');

  lines.push('export const EVENT_ADAPTER_TIER = {');
  for (const ev of proto.event_kinds ?? []) {
    lines.push(`  ${ev.name.toUpperCase()}: '${ev.adapterTier}',`);
  }
  lines.push('} as const;');
  lines.push('export type EVENT_ADAPTER_TIER = typeof EVENT_ADAPTER_TIER;');
  lines.push('');

  lines.push('export const ELEMENT_KIND = {');
  for (const ek of proto.element_kinds ?? []) {
    const key = ek.name.replace(/_/g, '-');
    lines.push(`  '${key}': ${ek.value},`);
  }
  lines.push('} as const;');
  lines.push('export type ELEMENT_KIND = typeof ELEMENT_KIND;');
  lines.push('');

  lines.push('export const UNSET_KIND = {');
  for (const uk of proto.unset_kinds ?? []) {
    lines.push(`  ${toCamelCase(uk.name)}: ${uk.value},`);
  }
  lines.push('} as const;');
  lines.push('export type UNSET_KIND = typeof UNSET_KIND;');
  lines.push('');

  lines.push('export const MODIFIER = {');
  for (const mk of proto.modifier_keys ?? []) {
    lines.push(`  ${mk.name.toUpperCase()}: ${mk.value},`);
  }
  lines.push('} as const;');
  lines.push('export type MODIFIER = typeof MODIFIER;');
  lines.push('');

  for (const en of proto.enums ?? []) {
    const constName = en.name.toUpperCase();
    lines.push(`export const ${constName} = {`);
    for (const v of en.values ?? []) {
      if (en.string_values === true) {
        lines.push(`  ${v.name}: '${v.value}',`);
      } else {
        lines.push(`  ${toCamelCase(v.name)}: ${v.value},`);
      }
    }
    lines.push('} as const;');
    lines.push(`export type ${constName} = typeof ${constName};`);
    lines.push('');
  }

  lines.push('export const UNIT_CODE = DIMENSION_UNIT;');
  lines.push('');

  function typeSlots(type, count) {
    if (count && count !== 0) return count;
    const typeDef = (proto.types ?? []).find((t) => t.name === type);
    if (typeDef) return typeDef.raw_slots;
    return 1;
  }

  const opSlots = (proto.opcodes ?? []).map((op) => {
    let slots = 0;
    for (const p of op.params ?? []) {
      slots += typeSlots(p.type, p.count);
    }
    return slots;
  });
  lines.push(`export const OP_SLOTS: readonly number[] = [${opSlots.join(', ')}];`);
  lines.push('');

  lines.push('// ── イベントペイロード型 ─────────────────────────────────────────────────');
  lines.push('');

  const eventUnionLines = [];
  for (const ev of proto.event_kinds ?? []) {
    const params = ev.params ?? [];
    const fields = params.map((p) => {
      let tsType;
      switch (p.type) {
        case 'element_id':
        case 'f32':
        case 'f64':
        case 'u32':
        case 'usize':
          tsType = 'number';
          break;
        case 'string':
          tsType = 'string';
          break;
        default:
          tsType = 'number';
      }
      return `${toCamelCase(p.name)}: ${tsType}`;
    });
    const allFields = [`kind: '${ev.name}'`, `value: ${ev.value}`, ...fields];
    eventUnionLines.push(`  | { ${allFields.join('; ')} }`);
  }

  lines.push('export type EventPayload =');
  lines.push(eventUnionLines.join('\n'));
  lines.push(';');
  lines.push('');

  lines.push('export function parseEvent(ev: unknown[]): EventPayload {');
  lines.push('  const kind = ev[0] as number;');
  lines.push('  switch (kind) {');

  for (const ev of proto.event_kinds ?? []) {
    const params = ev.params ?? [];
    lines.push(`    case ${ev.value}: { // ${ev.name}`);
    const fieldAssignments = params.map((p, idx) => {
      const fieldName = toCamelCase(p.name);
      const cast = p.type === 'string' ? 'string' : 'number';
      return `${fieldName}: ev[${idx + 1}] as ${cast}`;
    });
    const returnFields = [`kind: '${ev.name}' as const`, `value: ${ev.value}`, ...fieldAssignments];
    lines.push(`      return { ${returnFields.join(', ')} };`);
    lines.push('    }');
  }

  lines.push('    default:');
  lines.push('      throw new Error(`parseEvent: unknown event kind ${kind}`);');
  lines.push('  }');
  lines.push('}');
  lines.push('');

  mkdirSync(outDir, { recursive: true });
  writeFileSync(outPath, lines.join('\n'), 'utf8');
}
