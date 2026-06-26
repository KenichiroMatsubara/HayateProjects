/** Load Hayate Protocol Contract JSON spec (proto/spec/*.json). */

import { readFileSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';

const __dirname = dirname(fileURLToPath(import.meta.url));

/** Default spec directory bundled with @hayate/protocol-spec. */
export const DEFAULT_SPEC_DIR = join(__dirname, '../spec');

export const SPEC_SECTIONS = [
  'types',
  'enums',
  'opcodes',
  'style_tags',
  'event_kinds',
  'element_kinds',
  'element_properties',
  'unset_kinds',
  'modifier_keys',
  'pseudo_states',
];

export function loadProtocolSpec(specDir = DEFAULT_SPEC_DIR) {
  const proto = {};
  for (const section of SPEC_SECTIONS) {
    const path = join(specDir, `${section}.json`);
    proto[section] = JSON.parse(readFileSync(path, 'utf8'));
  }
  // manifest は wire の protocol version（整合トークン）の source of truth。section とは別に
  // 読み込み、generator が `PROTOCOL_VERSION` として焼き出せるようにする（#530）。
  proto.manifest = JSON.parse(readFileSync(join(specDir, 'manifest.json'), 'utf8'));
  return proto;
}

export function toCamelCase(s) {
  return s.replace(/_([a-z])/g, (_, c) => c.toUpperCase());
}

export function tagToPatchKey(name) {
  const lower = name.toLowerCase();
  return lower.replace(/_([a-z])/g, (_, c) => c.toUpperCase());
}

export function toKebabCase(camel) {
  return camel.replace(/[A-Z]/g, (m) => `-${m.toLowerCase()}`);
}
