import { writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { loadProtocolSpec } from '@hayate/protocol-spec/load';

const outDir = join(
  dirname(fileURLToPath(import.meta.url)),
  '../../packages/renderer-protocol/src/generated',
);
const outPath = join(outDir, 'event-kind.ts');

export function generateEventKind() {
  const proto = loadProtocolSpec();

  const kinds = (proto.event_kinds ?? [])
    .map((ev) => ev.interactionKind)
    .filter((kind) => kind !== null && kind !== undefined);

  const lines = [
    '// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと',
    '// 生成元: @hayate/protocol-spec',
    '',
    '/** 要素レベルの Interaction Event 種別（event_kinds.json の `interactionKind` を参照）。 */',
    'export type EventKind =',
    ...kinds.map((kind, i) => `  | '${kind}'${i === kinds.length - 1 ? ';' : ''}`),
    '',
  ];

  mkdirSync(outDir, { recursive: true });
  writeFileSync(outPath, lines.join('\n'), 'utf8');
}
