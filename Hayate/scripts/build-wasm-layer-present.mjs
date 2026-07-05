// scripts/build-wasm-layer-present.mjs — hayate-adapter-web を `layer-present` feature 有効で
// wasm-pack ビルドする（#697）。
//
// `layer-present`（#690・ADR-0125/0127、既定 OFF）は cargo feature なのでランタイムには切り
// 替えられず、ON/OFF は別バイナリになる。`build-wasm.mjs` の既定ビルド（`wasm-pkgs/pkg`）は
// layer-present OFF のまま据え置き、本スクリプトは同じ default features に `layer-present` を
// 足しただけの ON 版を `wasm-pkgs/pkg-layer-present` へ別出力する（比較対象を1 feature 差に絞る
// ため、既定ビルドと同じ `--target web` 呼び出しに `--features layer-present` を足すだけ）。
// target dir も分離し、既定ビルドの incremental キャッシュを汚さない。
import { spawnSync } from 'node:child_process';
import { copyFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const ROOT_DIR = join(SCRIPT_DIR, '..');
const CRATE_DIR = join(ROOT_DIR, 'crates', 'platform', 'web');
const OUT_DIR = join(ROOT_DIR, 'wasm-pkgs', 'pkg-layer-present');
const TARGET_DIR = join(ROOT_DIR, 'target', 'wasm-layer-present');

const BOLD = '\x1b[1m';
const GREEN = '\x1b[0;32m';
const CYAN = '\x1b[0;36m';
const RESET = '\x1b[0m';

function run(cmd, args, targetDir) {
  const bin = process.platform === 'win32' ? `${cmd}.exe` : cmd;
  const env = targetDir ? { ...process.env, CARGO_TARGET_DIR: targetDir } : process.env;
  const result = spawnSync(bin, args, { stdio: 'inherit', env });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

// build-wasm.mjs と同じ正規化 package.json（wasm-pack のバージョン間で揺れる出力を固定する）。
const PKG_JSON = `${JSON.stringify(
  {
    name: 'hayate-adapter-web',
    type: 'module',
    description: 'Hayate — GPU-native UI substrate (layer-present feature, #697 E2E harness only)',
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

console.log(`${BOLD}━━━ hayate WASM build (layer-present, #697) ━━━${RESET}`);
console.log(`  root : ${ROOT_DIR}`);
console.log(`  crate: ${CRATE_DIR}`);
console.log(`  out  : ${OUT_DIR}`);
console.log();

copyFileSync(join(ROOT_DIR, 'LICENSE'), join(CRATE_DIR, 'LICENSE'));

console.log(`${CYAN}▶ wasm-pack build --target web (default features + layer-present)...${RESET}`);
run(
  'wasm-pack',
  ['build', CRATE_DIR, '--target', 'web', '--out-dir', OUT_DIR, '--', '--features', 'layer-present'],
  TARGET_DIR,
);
writeFileSync(join(OUT_DIR, '.gitignore'), '*\n!package.json\n');
writeFileSync(join(OUT_DIR, 'package.json'), PKG_JSON);
console.log();

console.log(`${GREEN}${BOLD}✓ Done!${RESET}`);
console.log('  pkg-layer-present → wasm-pkgs/pkg-layer-present/');
console.log('  consumed only by Tsubame/examples/todo e2e (vite.config.e2e-layer-present.ts, #697)');
