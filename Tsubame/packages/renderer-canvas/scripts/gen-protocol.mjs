#!/usr/bin/env node
// gen-protocol.mjs — reads Hayate/proto/protocol.yaml and writes src/protocol.ts
// Hand-written YAML parser: block-only format, fixed indentation.

import { readFileSync, writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const yamlPath = join(__dirname, '../../../../Hayate/proto/protocol.yaml');
const outPath = join(__dirname, '../src/protocol.ts');

const yaml = readFileSync(yamlPath, 'utf8');

// ── Simple line-by-line YAML parser ──────────────────────────────────────────
// Handles the exact fixed format of protocol.yaml (block-only, no inline braces).

function parseYaml(text) {
  const result = {};
  let section = null;
  let currentItem = null;
  let currentParam = null;

  function setKv(obj, key, rawVal) {
    // Strip surrounding double quotes
    const val = rawVal.replace(/^"(.*)"$/, '$1');
    obj[key] = val;
  }

  for (const rawLine of text.split('\n')) {
    // Strip inline comments and trailing whitespace
    const noComment = rawLine.replace(/#.*$/, '').trimEnd();
    if (!noComment.trim()) continue;

    const indent = noComment.length - noComment.trimStart().length;
    const content = noComment.trim();

    // ── indent 0: section header ──────────────────────────────────────────
    if (indent === 0) {
      if (content.endsWith(':')) {
        const name = content.slice(0, -1);
        if (name !== 'version') {
          section = name;
          result[section] = [];
        }
      }
      currentItem = null;
      currentParam = null;
      continue;
    }

    if (!section) continue;

    // ── indent 2: list item start ("  - name: VALUE") ─────────────────────
    if (indent === 2) {
      if (content.startsWith('- ')) {
        const rest = content.slice(2);
        currentItem = {};
        result[section].push(currentItem);
        currentParam = null;
        if (rest.includes(':')) {
          const colonIdx = rest.indexOf(':');
          const k = rest.slice(0, colonIdx).trim();
          const v = rest.slice(colonIdx + 1).trim();
          setKv(currentItem, k, v);
        }
      }
      continue;
    }

    if (!currentItem) continue;

    // ── indent 4: item property OR sub-list key ───────────────────────────
    if (indent === 4) {
      if (content.startsWith('- ')) {
        // This shouldn't happen at indent=4 in our format, but handle gracefully
        const rest = content.slice(2);
        currentParam = {};
        if (!Array.isArray(currentItem._cur_list)) currentItem._cur_list = [];
        currentItem._cur_list.push(currentParam);
        if (rest.includes(':')) {
          const colonIdx = rest.indexOf(':');
          const k = rest.slice(0, colonIdx).trim();
          const v = rest.slice(colonIdx + 1).trim();
          setKv(currentParam, k, v);
        }
      } else if (content.includes(':')) {
        const colonIdx = content.indexOf(':');
        const key = content.slice(0, colonIdx).trim();
        const val = content.slice(colonIdx + 1).trim();
        if (val === '') {
          // This starts a new sub-list (params:, values:, fields:)
          currentItem[key] = [];
          currentItem._active_list = key;
          currentParam = null;
        } else {
          const cleanVal = val.replace(/^"(.*)"$/, '$1');
          currentItem[key] = cleanVal;
        }
      }
      continue;
    }

    // ── indent 6: sub-list item start ("      - name: VALUE") ───────────
    if (indent === 6) {
      if (content.startsWith('- ')) {
        const rest = content.slice(2);
        currentParam = {};
        const listKey = currentItem._active_list;
        if (listKey && Array.isArray(currentItem[listKey])) {
          currentItem[listKey].push(currentParam);
        }
        if (rest.includes(':')) {
          const colonIdx = rest.indexOf(':');
          const k = rest.slice(0, colonIdx).trim();
          const v = rest.slice(colonIdx + 1).trim();
          setKv(currentParam, k, v);
        }
      } else if (content.includes(':') && currentParam) {
        const colonIdx = content.indexOf(':');
        const k = content.slice(0, colonIdx).trim();
        const v = content.slice(colonIdx + 1).trim();
        setKv(currentParam, k, v);
      }
      continue;
    }

    // ── indent 8: sub-list item property ──────────────────────────────────
    if (indent === 8 && currentParam) {
      if (content.includes(':')) {
        const colonIdx = content.indexOf(':');
        const k = content.slice(0, colonIdx).trim();
        const v = content.slice(colonIdx + 1).trim();
        setKv(currentParam, k, v);
      }
    }
  }

  // Clean up internal tracking keys
  for (const items of Object.values(result)) {
    for (const item of items) {
      delete item._active_list;
      delete item._cur_list;
    }
  }

  return result;
}

const proto = parseYaml(yaml);

// ── Code generation ───────────────────────────────────────────────────────────

function toCamelCase(s) {
  return s.replace(/_([a-z])/g, (_, c) => c.toUpperCase());
}

const lines = [
  '// AUTO-GENERATED by scripts/gen-protocol.mjs — DO NOT EDIT',
  '// Source: Hayate/proto/protocol.yaml',
  '',
];

// ── OP constants ──────────────────────────────────────────────────────────────
lines.push('export const OP = {');
for (const op of proto.opcodes ?? []) {
  lines.push(`  ${op.name}: ${op.value},`);
}
lines.push('} as const;');
lines.push('export type OP = typeof OP;');
lines.push('');

// ── TAG constants ─────────────────────────────────────────────────────────────
lines.push('export const TAG = {');
for (const tag of proto.style_tags ?? []) {
  lines.push(`  ${tag.name}: ${tag.value},`);
}
lines.push('} as const;');
lines.push('export type TAG = typeof TAG;');
lines.push('');

// ── EVENT_KIND constants ───────────────────────────────────────────────────────
lines.push('export const EVENT_KIND = {');
for (const ev of proto.event_kinds ?? []) {
  lines.push(`  ${ev.name.toUpperCase()}: ${ev.value},`);
}
lines.push('} as const;');
lines.push('export type EVENT_KIND = typeof EVENT_KIND;');
lines.push('');

// ── ELEMENT_KIND constants ────────────────────────────────────────────────────
// Keys use kebab-case to match the ElementKind type in renderer-protocol
// (e.g. 'text-input', 'scroll-view') so it can be used as Record<ElementKind, number>.
lines.push('export const ELEMENT_KIND = {');
for (const ek of proto.element_kinds ?? []) {
  // Convert snake_case (text_input) to kebab-case (text-input) for ElementKind compatibility
  const key = ek.name.replace(/_/g, '-');
  lines.push(`  '${key}': ${ek.value},`);
}
lines.push('} as const;');
lines.push('export type ELEMENT_KIND = typeof ELEMENT_KIND;');
lines.push('');

// ── UNSET_KIND constants ──────────────────────────────────────────────────────
lines.push('export const UNSET_KIND = {');
for (const uk of proto.unset_kinds ?? []) {
  lines.push(`  ${toCamelCase(uk.name)}: ${uk.value},`);
}
lines.push('} as const;');
lines.push('export type UNSET_KIND = typeof UNSET_KIND;');
lines.push('');

// ── MODIFIER constants ────────────────────────────────────────────────────────
lines.push('export const MODIFIER = {');
for (const mk of proto.modifier_keys ?? []) {
  lines.push(`  ${mk.name.toUpperCase()}: ${mk.value},`);
}
lines.push('} as const;');
lines.push('export type MODIFIER = typeof MODIFIER;');
lines.push('');

// ── Enum constants ────────────────────────────────────────────────────────────
for (const en of proto.enums ?? []) {
  const constName = en.name.toUpperCase();
  lines.push(`export const ${constName} = {`);
  for (const v of en.values ?? []) {
    if (en.string_values === 'true') {
      lines.push(`  ${v.name}: '${v.value}',`);
    } else {
      lines.push(`  ${toCamelCase(v.name)}: ${v.value},`);
    }
  }
  lines.push('} as const;');
  lines.push(`export type ${constName} = typeof ${constName};`);
  lines.push('');
}

// ── UNIT_CODE (alias for DIMENSION_UNIT, backwards compat) ───────────────────
lines.push('export const UNIT_CODE = DIMENSION_UNIT;');
lines.push('');

// ── OP_SLOTS array ────────────────────────────────────────────────────────────
function typeSlots(type, count) {
  if (count && count !== '0') return parseInt(count, 10);
  // types section
  const typeDef = (proto.types ?? []).find(t => t.name === type);
  if (typeDef) return parseInt(typeDef.raw_slots, 10);
  return 1; // primitives
}

const opSlots = (proto.opcodes ?? []).map(op => {
  let slots = 0;
  for (const p of op.params ?? []) {
    slots += typeSlots(p.type, p.count);
  }
  return slots;
});
lines.push(`export const OP_SLOTS: readonly number[] = [${opSlots.join(', ')}];`);
lines.push('');

// ── EventPayload discriminated union ─────────────────────────────────────────
lines.push('// ── Event payload types ─────────────────────────────────────────────────────');
lines.push('');

const eventUnionLines = [];
for (const ev of proto.event_kinds ?? []) {
  const params = ev.params ?? [];
  const fields = params.map(p => {
    let tsType;
    switch (p.type) {
      case 'element_id': tsType = 'number'; break;
      case 'f32': case 'f64': case 'u32': case 'usize': tsType = 'number'; break;
      case 'string': tsType = 'string'; break;
      default: tsType = 'number';
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

// ── parseEvent function ───────────────────────────────────────────────────────
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
  lines.push(`    }`);
}

lines.push(`    default:`);
lines.push(`      throw new Error(\`parseEvent: unknown event kind \${kind}\`);`);
lines.push('  }');
lines.push('}');
lines.push('');

// Write output
mkdirSync(dirname(outPath), { recursive: true });
writeFileSync(outPath, lines.join('\n'), 'utf8');
console.log(`Generated ${outPath}`);
