// scripts/build-wasm.mjs — hayate-adapter-web を wasm-pack でビルドする（ネイティブ Windows / cross-platform）。
//
// かつては build-wasm.sh を `bash` 経由で呼んでいたが、Windows では `bash` が WSL の
// ランチャ(App Execution Alias)に解決され、cargo の target が /mnt/d (DrvFs) 上に置かれて
// インクリメンタルコンパイルの確定 rename が `Permission denied (os error 13)` で失敗 →
// 毎回コールドビルド化していた。node から cargo/wasm-pack をネイティブ実行すれば target は
// D: の NTFS に直接書かれ、incremental が正常に効く（強制フルは `pnpm --filter hayate clean`）。
import { spawnSync } from 'node:child_process';
import { copyFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const ROOT_DIR = join(SCRIPT_DIR, '..');
const CRATE_DIR = join(ROOT_DIR, 'crates', 'platform', 'web');
const OUT_DIR = join(ROOT_DIR, 'wasm-pkgs', 'pkg');
const OUT_DIR_CPU = join(ROOT_DIR, 'wasm-pkgs', 'pkg-tiny-skia');
const OUT_DIR_NULL = join(ROOT_DIR, 'wasm-pkgs', 'pkg-null');

// backend ごとに CARGO_TARGET_DIR を分離する。3つの wasm-pack は default / backend-tiny-skia /
// backend-null と排他的な feature 構成なので、同一 target を共有すると毎回相手を無効化して
// フル再コンパイルになる（feature 綱引き）。target を分ければ各 backend が自分の incremental
// キャッシュを保持し、変更の無い backend は cargo コンパイルがフルキャッシュされる（残りは
// wasm-bindgen / wasm-opt の固定後処理のみ）。すべて gitignore 済みの target/ 配下に置く。
// cargo check は default features なので pkg と同じ target を共有してキャッシュを再利用する。
const TARGET_DIR = join(ROOT_DIR, 'target', 'wasm');
const TARGET_DIR_CPU = join(ROOT_DIR, 'target', 'wasm-tiny-skia');
const TARGET_DIR_NULL = join(ROOT_DIR, 'target', 'wasm-null');

const BOLD = '\x1b[1m';
const GREEN = '\x1b[0;32m';
const CYAN = '\x1b[0;36m';
const RESET = '\x1b[0m';

/**
 * cargo/wasm-pack をネイティブ実行。失敗したら set -e 相当でそのまま終了する。
 * shell:true は引数が未エスケープのまま連結され DEP0190 警告も出るので使わない。Windows では
 * shell 無し spawn が PATHEXT 解決をしないため、rustup 配布の `.exe` を明示する。
 * targetDir を渡すと CARGO_TARGET_DIR を上書きし、backend 別に incremental キャッシュを分ける。
 */
function run(cmd, args, targetDir) {
  const bin = process.platform === 'win32' ? `${cmd}.exe` : cmd;
  const env = targetDir ? { ...process.env, CARGO_TARGET_DIR: targetDir } : process.env;
  const result = spawnSync(bin, args, { stdio: 'inherit', env });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

// wasm-pack は毎回 package.json を再生成するが、その出力レイアウトはバージョン間で揺れる
// （0.15 では description/repository/files/sideEffects が落ちる）。これが追跡対象の
// package.json に毎回ノイズ差分を生み、IDE のビルド/同期のたびに差分が出る原因だった。
// これらは公開せず社内の `file:` 依存としてのみ消費されるので、ここで正規版を固定して
// wasm-pack の出力を上書きする。`sideEffects` は wasm-bindgen の snippets をバンドラに
// tree-shake されないために必須なので残す。
const PKG_JSON = `${JSON.stringify(
  {
    name: 'hayate-adapter-web',
    type: 'module',
    description: 'Hayate — GPU-native UI substrate',
    version: '0.1.0',
    license: 'Apache-2.0',
    repository: {
      type: 'git',
      url: 'https://github.com/KenichiroMatsubara/HayateProjects',
    },
    files: ['hayate_adapter_web_bg.wasm', 'hayate_adapter_web.js', 'hayate_adapter_web.d.ts'],
    main: 'hayate_adapter_web.js',
    types: 'hayate_adapter_web.d.ts',
    sideEffects: ['./snippets/*'],
  },
  null,
  2,
)}\n`;

function finalizePkg(dir) {
  writeFileSync(join(dir, '.gitignore'), '*\n!package.json\n');
  writeFileSync(join(dir, 'package.json'), PKG_JSON);
}

console.log(`${BOLD}━━━ hayate WASM build ━━━${RESET}`);
console.log(`  root : ${ROOT_DIR}`);
console.log(`  crate: ${CRATE_DIR}`);
console.log(`  out  : ${OUT_DIR}`);
console.log();

// wasm-pack expects LICENSE beside the crate manifest.
copyFileSync(join(ROOT_DIR, 'LICENSE'), join(CRATE_DIR, 'LICENSE'));

// ── Step 1: cargo check (wasm32) ─────────────────────────────────────────────
console.log(`${CYAN}▶ cargo check (wasm32-unknown-unknown)...${RESET}`);
run(
  'cargo',
  [
    'check',
    '--manifest-path',
    join(ROOT_DIR, 'Cargo.toml'),
    '-p',
    'hayate-core',
    '-p',
    'hayate-adapter-web',
    '--target',
    'wasm32-unknown-unknown',
  ],
  TARGET_DIR,
);
console.log();

// ── Step 2: wasm-pack build ──────────────────────────────────────────────────
console.log(`${CYAN}▶ wasm-pack build --target web...${RESET}`);
run('wasm-pack', ['build', CRATE_DIR, '--target', 'web', '--out-dir', OUT_DIR], TARGET_DIR);
finalizePkg(OUT_DIR);
console.log();

// ── Step 3: wasm-pack build (tiny-skia CPU backend) ─────────────────────────
console.log(`${CYAN}▶ wasm-pack build --target web (backend-tiny-skia)...${RESET}`);
run(
  'wasm-pack',
  [
    'build',
    CRATE_DIR,
    '--target',
    'web',
    '--out-dir',
    OUT_DIR_CPU,
    '--',
    '--no-default-features',
    '--features',
    'backend-tiny-skia',
  ],
  TARGET_DIR_CPU,
);
finalizePkg(OUT_DIR_CPU);
console.log();

// ── Step 4: wasm-pack build (null backend — C3 codec integration tests) ─────
console.log(`${CYAN}▶ wasm-pack build --target web (backend-null)...${RESET}`);
run(
  'wasm-pack',
  [
    'build',
    CRATE_DIR,
    '--target',
    'web',
    '--out-dir',
    OUT_DIR_NULL,
    '--',
    '--no-default-features',
    '--features',
    'backend-null',
  ],
  TARGET_DIR_NULL,
);
finalizePkg(OUT_DIR_NULL);
console.log();

console.log(`${GREEN}${BOLD}✓ Done!${RESET}`);
console.log('  pkg           → wasm-pkgs/pkg/');
console.log('  pkg-tiny-skia → wasm-pkgs/pkg-tiny-skia/');
console.log('  pkg-null      → wasm-pkgs/pkg-null/');
console.log('  consumed by Tsubame renderer-hayate (file: deps in wasm-pkgs/*)');
