#!/usr/bin/env node
/** Validate proto/spec/*.json against JSON Schema (ADR-0053). */

import Ajv2020 from 'ajv/dist/2020.js';
import { readFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

import { SPEC_SECTIONS } from './load-spec.mjs';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const specDir = join(root, 'spec');
const schemaDir = join(specDir, 'schema');

const ajv = new Ajv2020({ allErrors: true, strict: false, validateSchema: false });

function loadSchema(name) {
  const path = join(schemaDir, name);
  return JSON.parse(readFileSync(path, 'utf8'));
}

const compiled = new Map();

function validatorFor(name) {
  if (!compiled.has(name)) {
    compiled.set(name, ajv.compile(loadSchema(name)));
  }
  return compiled.get(name);
}

const validators = {
  types: validatorFor('type.schema.json'),
  enums: validatorFor('enum.schema.json'),
  opcodes: validatorFor('entry.schema.json'),
  style_tags: validatorFor('style_tag.schema.json'),
  event_kinds: validatorFor('event_kind.schema.json'),
  element_kinds: validatorFor('simple_entry.schema.json'),
  element_properties: validatorFor('element_property.schema.json'),
  unset_kinds: validatorFor('unset_kind.schema.json'),
  modifier_keys: validatorFor('modifier_key.schema.json'),
  pseudo_states: validatorFor('pseudo_state.schema.json'),
};

const validateManifest = validatorFor('manifest.schema.json');

function readJson(path) {
  return JSON.parse(readFileSync(path, 'utf8'));
}

function assertValid(validator, data, label) {
  if (!validator(data)) {
    const detail = (validator.errors ?? [])
      .map((e) => `${e.instancePath || '/'} ${e.message}`)
      .join('; ');
    throw new Error(`${label}: ${detail}`);
  }
}

const manifest = readJson(join(specDir, 'manifest.json'));
assertValid(validateManifest, manifest, 'manifest.json');

const manifestSections = new Set(manifest.sections);
for (const section of SPEC_SECTIONS) {
  if (!manifestSections.has(section)) {
    throw new Error(`manifest.json missing section: ${section}`);
  }
}

for (const section of SPEC_SECTIONS) {
  const path = join(specDir, `${section}.json`);
  const data = readJson(path);
  if (!Array.isArray(data)) {
    throw new Error(`${section}.json must be an array`);
  }
  const validator = validators[section];
  for (let i = 0; i < data.length; i++) {
    assertValid(validator, data[i], `${section}.json[${i}]`);
    if (section === 'modifier_keys') {
      const { value } = data[i];
      if ((value & (value - 1)) !== 0) {
        throw new Error(
          `modifier_keys.json[${i}]: value ${value} must be a single-bit mask (power of two)`,
        );
      }
    }
    if (section === 'pseudo_states') {
      const priorities = data.map((e) => e.priority);
      const unique = new Set(priorities);
      if (unique.size !== priorities.length) {
        throw new Error('pseudo_states.json: priority values must be unique');
      }
    }
  }
}

console.log(
  `Validated ${SPEC_SECTIONS.length} spec sections + manifest against JSON Schema in ${specDir}`,
);
