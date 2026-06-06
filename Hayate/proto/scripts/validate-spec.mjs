#!/usr/bin/env node
/** Validate proto/spec/*.json structure (ADR-0053). */

import { loadProtocolSpec, SPEC_SECTIONS } from './load-spec.mjs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const specDir = join(dirname(fileURLToPath(import.meta.url)), '../spec');
const proto = loadProtocolSpec(specDir);

function assertArray(name, value) {
  if (!Array.isArray(value)) {
    throw new Error(`${name} must be an array`);
  }
}

function assertEntry(entry, section) {
  if (typeof entry.name !== 'string' || entry.name.length === 0) {
    throw new Error(`${section}: entry missing name`);
  }
  if (typeof entry.value !== 'number') {
    throw new Error(`${section}.${entry.name}: value must be a number`);
  }
}

for (const section of SPEC_SECTIONS) {
  assertArray(section, proto[section]);
}

for (const t of proto.types) {
  if (typeof t.name !== 'string' || typeof t.raw_slots !== 'number') {
    throw new Error(`types.${t.name ?? '?'}: invalid type definition`);
  }
  if (!Array.isArray(t.fields)) throw new Error(`types.${t.name}: fields must be array`);
}

for (const en of proto.enums) {
  if (typeof en.name !== 'string' || !Array.isArray(en.values)) {
    throw new Error(`enums.${en.name ?? '?'}: invalid enum definition`);
  }
}

for (const section of ['opcodes', 'style_tags', 'event_kinds']) {
  for (const entry of proto[section]) {
    assertEntry(entry, section);
    if (entry.params !== undefined && !Array.isArray(entry.params)) {
      throw new Error(`${section}.${entry.name}: params must be array`);
    }
  }
}

for (const section of ['element_kinds', 'unset_kinds', 'modifier_keys']) {
  for (const entry of proto[section]) {
    assertEntry(entry, section);
  }
}

console.log(`Validated ${SPEC_SECTIONS.length} spec sections in ${specDir}`);
