import { writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { loadProtocolSpec, tagToPatchKey } from '@torimi/hayate-protocol-spec/load';

const outDir = join(
  dirname(fileURLToPath(import.meta.url)),
  '../../packages/renderer-protocol/src/generated',
);
const outPath = join(outDir, 'style-channel.ts');

/** Spec element-kind name (snake_case) → Tsubame ElementKind union (kebab-case). */
function kindToUnion(name) {
  return name.replace(/_/g, '-');
}

export function generateStyleChannel() {
  const proto = loadProtocolSpec();

  const textLocalKeys = (proto.style_tags ?? [])
    .filter((t) => t.inherit === 'text-local')
    .map((t) => tagToPatchKey(t.name));

  const carriers = (proto.element_kinds ?? [])
    .filter((k) => k.carriesTextLocal)
    .map((k) => kindToUnion(k.name));

  const lines = [
    '// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと',
    '// 生成元: @torimi/hayate-protocol-spec（style_tags.inherit / element_kinds.carriesTextLocal）',
    '',
    "import type { StylePatch } from '../style.js';",
    "import type { ElementKind } from '../element.js';",
    '',
    '/** チャネル1の text-local スタイルキー（Style Channel; ADR-0065 / ADR-0002）。 */',
    `const TEXT_LOCAL_KEYS: ReadonlySet<keyof StylePatch> = new Set(${JSON.stringify(textLocalKeys)} as (keyof StylePatch)[]);`,
    '',
    '/** `key` がチャネル1の text-local スタイルかどうか。 */',
    'export function isTextLocal(key: string): boolean {',
    '  return TEXT_LOCAL_KEYS.has(key as keyof StylePatch);',
    '}',
    '',
    '/** チャネル1のスタイルを CSS として保持する要素種別（Text-Local Carrier）。 */',
    `const TEXT_LOCAL_CARRIERS: ReadonlySet<ElementKind> = new Set(${JSON.stringify(carriers)} as ElementKind[]);`,
    '',
    '/** `kind` がチャネル1の text-local スタイルを CSS として保持するかどうか。 */',
    'export function carriesTextLocal(kind: ElementKind): boolean {',
    '  return TEXT_LOCAL_CARRIERS.has(kind);',
    '}',
    '',
  ];

  mkdirSync(outDir, { recursive: true });
  writeFileSync(outPath, lines.join('\n'), 'utf8');
}
