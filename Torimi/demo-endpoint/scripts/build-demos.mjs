// デモ App Bundle（Android ホスト向け・Hermes 降格済み）をビルドして public/ に集める。
// ローカルは `pnpm build:demos && pnpm dev`、本番はリリース lockstep CI が AAB と同じ
// コミットから同じスクリプトで作る（ADR-0003）。
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import demosSource from '../src/demos.json' with { type: 'json' };

/** 各デモパッケージ側の、Hermes 降格済み単一バンドル（Native Host 向け）を作るスクリプト名（ADR-0112 の流儀）。 */
const NATIVE_BUILD_SCRIPT = 'torimi:native:build';

const packageRoot = dirname(dirname(fileURLToPath(import.meta.url)));
const repoRoot = join(packageRoot, '..', '..');

for (const demo of demosSource.demos) {
  const { workspacePackage, artifactPath } = demo.source;
  console.log(`build:demos: ${demo.name} (${workspacePackage})`);
  execFileSync('pnpm', ['--filter', workspacePackage, 'run', NATIVE_BUILD_SCRIPT], {
    cwd: repoRoot,
    stdio: 'inherit',
  });
  const target = join(packageRoot, 'public', ...demo.bundleUrl.split('/').filter(Boolean));
  mkdirSync(dirname(target), { recursive: true });
  copyFileSync(join(repoRoot, artifactPath), target);
  console.log(`build:demos: -> ${target}`);
}
