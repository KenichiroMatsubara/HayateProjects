import { writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';

const outPath = join(dirname(fileURLToPath(import.meta.url)), '../generated/index.ts');

export function writeIndex() {
  const lines = [
    '// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと',
    '',
    "export * from './protocol.js';",
    "export * from './codec.js';",
    "export * from './catalog.js';",
    "export * from './delivery.js';",
    '',
  ];
  mkdirSync(dirname(outPath), { recursive: true });
  writeFileSync(outPath, lines.join('\n'), 'utf8');
}
