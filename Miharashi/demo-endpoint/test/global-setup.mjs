// デモバンドル（public/ 配下の静的アセット）はビルド成果物で、テストはそれに依存させない。
// 各エントリの bundleUrl 位置にプレースホルダ JS を用意して hermetic にする（実ビルド済みなら
// 触らない）。
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import demosSource from '../src/demos.json' with { type: 'json' };

const packageRoot = dirname(dirname(fileURLToPath(import.meta.url)));

export default function setup() {
  for (const demo of demosSource.demos) {
    const assetPath = join(packageRoot, 'public', ...demo.bundleUrl.split('/').filter(Boolean));
    if (existsSync(assetPath)) continue;
    mkdirSync(dirname(assetPath), { recursive: true });
    writeFileSync(assetPath, `// placeholder demo bundle for tests: ${demo.name}\n`);
  }
}
