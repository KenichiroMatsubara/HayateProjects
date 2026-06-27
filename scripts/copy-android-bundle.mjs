// scripts/copy-android-bundle.mjs
// Tsubame Todo の Hermes 用バンドル（build:android の生成物）を Android APK の
// assets へコピーする。Gradle 内で pnpm を回すのは脆いため（ADR-0112 のメモ参照）、
// バンドル焼き込みはこのスクリプトで明示的に行う。
import { copyFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = new URL('../', import.meta.url);
const src = fileURLToPath(new URL('Tsubame/examples/todo/dist-android/tsubame.js', root));
const dest = fileURLToPath(
  new URL(
    'Hayate/crates/platform/mobile/android/android-app/app/src/main/assets/tsubame.js',
    root,
  ),
);

if (!existsSync(src)) {
  console.error(`✗ バンドルが見つかりません: ${src}`);
  console.error('  先に `pnpm --filter @tsubame/example-todo build:android` を実行してください。');
  process.exit(1);
}

copyFileSync(src, dest);
console.log(`✓ tsubame.js を assets へ同梱しました\n  ${src}\n→ ${dest}`);
