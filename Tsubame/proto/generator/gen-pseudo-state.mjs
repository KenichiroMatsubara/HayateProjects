import { writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { loadProtocolSpec } from '@hayate/protocol-spec/load';

const outDir = join(
  dirname(fileURLToPath(import.meta.url)),
  '../../packages/renderer-protocol/src/generated',
);
const outPath = join(outDir, 'pseudo-state.ts');

export function generatePseudoState() {
  const proto = loadProtocolSpec();
  const states = [...(proto.pseudo_states ?? [])].sort(
    (a, b) => a.priority - b.priority,
  );

  const keys = states.map((s) => s.cssPseudo);
  const codeEntries = states.map((s) => `  '${s.cssPseudo}': ${s.value},`);
  const priorityEntries = states.map(
    (s) => `  '${s.cssPseudo}': ${s.priority},`,
  );

  const lines = [
    '// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと',
    '// 生成元: @hayate/protocol-spec',
    '',
    "import type { StylePatch } from '../style.js';",
    '',
    `export const PSEUDO_STYLE_KEYS = ${JSON.stringify(keys)} as const;`,
    'export type PseudoStyleKey = (typeof PSEUDO_STYLE_KEYS)[number];',
    '',
    'export type PseudoStylePatch = Partial<Record<PseudoStyleKey, StylePatch>>;',
    '',
    'export const PSEUDO_STATE_CODE: Record<PseudoStyleKey, number> = {',
    ...codeEntries,
    '};',
    '',
    '/** カスケードの帯域順（昇順・後勝ち）。ワイヤーコードとは別物。 */',
    'export const PSEUDO_STATE_PRIORITY: Record<PseudoStyleKey, number> = {',
    ...priorityEntries,
    '};',
    '',
    '/** 優先度帯域でソートした擬似キー（focus < hover < active）。 */',
    `export const PSEUDO_STYLE_KEYS_BY_PRIORITY = ${JSON.stringify(keys)} as const satisfies readonly PseudoStyleKey[];`,
    '',
  ];

  mkdirSync(outDir, { recursive: true });
  writeFileSync(outPath, lines.join('\n'), 'utf8');
}
