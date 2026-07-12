import { writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { loadProtocolSpec } from '@torimi/hayate-protocol-spec/load';

const outDir = join(
  dirname(fileURLToPath(import.meta.url)),
  '../../packages/renderer-protocol/src/generated',
);
const outPath = join(outDir, 'element-kind.ts');

/** Spec element-kind name (snake_case) → Tsubame ElementKind union (kebab-case). */
function kindToUnion(name) {
  return name.replace(/_/g, '-');
}

/** Spec cursor enum value (snake_case) → CSS cursor keyword (kebab-case). */
function cursorToCss(name) {
  return name.replace(/_/g, '-');
}

export function generateElementKind() {
  const proto = loadProtocolSpec();

  const cursorEntries = (proto.element_kinds ?? [])
    .filter((k) => k.defaultCursor)
    .map((k) => [kindToUnion(k.name), cursorToCss(k.defaultCursor)]);
  const cursorDefaults = Object.fromEntries(cursorEntries);

  const userSelectEntries = (proto.element_kinds ?? [])
    .filter((k) => k.defaultUserSelect)
    .map((k) => [kindToUnion(k.name), k.defaultUserSelect]);
  const userSelectDefaults = Object.fromEntries(userSelectEntries);

  const lines = [
    '// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと',
    '// 生成元: @torimi/hayate-protocol-spec（element_kinds.defaultCursor / defaultUserSelect）',
    '',
    "import type { ElementKind, UserSelect } from '../element.js';",
    '',
    '/** 要素種別ごとの UA デフォルトカーソル（CSS キーワード）（ADR-0105）。Canvas',
    ' *  （Hayate コアの `resolve_cursor`）と共有する単一ソース。未指定 = デフォルトなし。 */',
    `const DEFAULT_CURSOR: Partial<Record<ElementKind, string>> = ${JSON.stringify(cursorDefaults)};`,
    '',
    '/** `cursor` が明示指定されていない場合の `kind` の UA デフォルトカーソル。指定時は undefined。 */',
    'export function elementKindDefaultCursor(kind: ElementKind): string | undefined {',
    '  return DEFAULT_CURSOR[kind];',
    '}',
    '',
    '/** 要素種別ごとの UA デフォルト `user-select`（ADR-0108）。Canvas（Hayate コアの',
    ' *  `default_user_select`）と共有する単一ソース。未指定 = `none`。 */',
    `const DEFAULT_USER_SELECT: Partial<Record<ElementKind, UserSelect>> = ${JSON.stringify(userSelectDefaults)};`,
    '',
    '/** 明示的な値が設定されていない場合の `kind` の UA デフォルト `user-select`（ADR-0108）。 */',
    "export function elementKindDefaultUserSelect(kind: ElementKind): UserSelect {",
    "  return DEFAULT_USER_SELECT[kind] ?? 'none';",
    '}',
    '',
  ];

  mkdirSync(outDir, { recursive: true });
  writeFileSync(outPath, lines.join('\n'), 'utf8');
}
