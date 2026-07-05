#!/usr/bin/env node
// scripts/build-wasm.mjs — hayate-adapter-web を wasm-pack でビルドする（ネイティブ Windows / cross-platform）。
//
// かつては build-wasm.sh を `bash` 経由で呼んでいたが、Windows では `bash` が WSL の
// ランチャ(App Execution Alias)に解決され、cargo の target が /mnt/d (DrvFs) 上に置かれて
// インクリメンタルコンパイルの確定 rename が `Permission denied (os error 13)` で失敗 →
// 毎回コールドビルド化していた。node から cargo/wasm-pack をネイティブ実行すれば target は
// D: の NTFS に直接書かれ、incremental が正常に効く（強制フルは `pnpm --filter hayate clean`）。
//
// どの backend をどの cargo feature でビルドするかは wasm-build-manifest.json が正本（#700）。
// 引数無し = マニフェストの includeInDefaultBuild 全部（旧 build-wasm.mjs 相当、4 backend）。
// 引数にターゲット名を渡すとそれだけをビルドする
// （例: `node build-wasm.mjs pkg-layer-present` で旧 build-wasm-layer-present.mjs 相当）。
// `--all` は includeInDefaultBuild を無視してマニフェストの全ターゲットをビルドする
// （#701: CI の Pages デプロイが使う — マニフェストにターゲットを追加してもこのモードの
// 呼び出し側 [CI workflow] は変更不要になる）。
import { spawnSync } from 'node:child_process';
import { copyFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  loadManifest,
  selectTargets,
  wasmPackArgsFor,
  outDirFor,
  targetDirFor,
  packageJsonFor,
  GITIGNORE_CONTENTS,
} from './wasm-manifest.mjs';

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const ROOT_DIR = join(SCRIPT_DIR, '..');

// cargo check だけは特定 backend に属さない固定の疎通確認なので、manifest のどの target
// とも独立して default features 用の target dir を使う（旧スクリプトと同じ場所）。
const CHECK_TARGET_DIR = join(ROOT_DIR, 'target', 'wasm');

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

function buildTarget(target, manifest, crateDir) {
  const outDir = outDirFor(target, ROOT_DIR);
  const targetDir = targetDirFor(target, ROOT_DIR);

  console.log(`${CYAN}▶ wasm-pack build --target web (${target.name})...${RESET}`);
  run('wasm-pack', wasmPackArgsFor(target, crateDir, outDir), targetDir);
  writeFileSync(join(outDir, '.gitignore'), GITIGNORE_CONTENTS);
  writeFileSync(join(outDir, 'package.json'), packageJsonFor(target, manifest));
  console.log();
}

const argv = process.argv.slice(2);
const all = argv.includes('--all');
const manifest = loadManifest();
const targets = selectTargets(
  manifest,
  argv.filter((a) => a !== '--all'),
  { all },
);
const crateDir = join(ROOT_DIR, manifest.crateDir);

console.log(`${BOLD}━━━ hayate WASM build ━━━${RESET}`);
console.log(`  root   : ${ROOT_DIR}`);
console.log(`  crate  : ${crateDir}`);
console.log(`  targets: ${targets.map((t) => t.name).join(', ')}`);
console.log();

// wasm-pack expects LICENSE beside the crate manifest.
copyFileSync(join(ROOT_DIR, 'LICENSE'), join(crateDir, 'LICENSE'));

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
  CHECK_TARGET_DIR,
);
console.log();

// ── Step 2+: wasm-pack build, one per selected target ────────────────────────
// backend ごとに CARGO_TARGET_DIR を分離する（wasm-manifest.mjs の targetDirFor）。
// ビルド対象同士は排他的/加算的な feature 構成なので、同一 target を共有すると毎回相手を
// 無効化してフル再コンパイルになる（feature 綱引き）。target を分ければ各 backend が自分の
// incremental キャッシュを保持し、変更の無い backend は cargo コンパイルがフルキャッシュ
// される（残りは wasm-bindgen / wasm-opt の固定後処理のみ）。すべて gitignore 済みの
// target/ 配下に置く。
for (const target of targets) {
  buildTarget(target, manifest, crateDir);
}

console.log(`${GREEN}${BOLD}✓ Done!${RESET}`);
for (const target of targets) {
  console.log(`  ${target.name.padEnd(18)} → ${target.outDir}/`);
}
