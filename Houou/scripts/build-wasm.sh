#!/usr/bin/env bash
# Houou の wasm をビルドして pnpm workspace パッケージ `wasm-pkgs/houou` を作る。
#
# Hayate の scripts/build-wasm.sh と同方針:
#   - wasm-opt（binaryen）は回さない。CI で binaryen の取得・実行に依存させないため。
#   - wasm-pack が毎回書き換える package.json は、ここで正規版に固定して上書きする
#     （バージョン間で出力レイアウトが揺れ、追跡対象の package.json に毎回ノイズ差分が出る）。
set -euo pipefail

# shellcheck source=/dev/null
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CRATE_DIR="$ROOT_DIR/crates/houou-wasm"
OUT_DIR="$ROOT_DIR/wasm-pkgs/houou"

wasm-pack build "$CRATE_DIR" \
  --target web \
  --out-dir "$OUT_DIR" \
  --out-name houou \
  --release

# 生成物そのものは追跡しない。package.json だけ残す。
printf '%s\n' '*' '!package.json' > "$OUT_DIR/.gitignore"

cat > "$OUT_DIR/package.json" <<'JSON'
{
  "name": "@houou/wasm",
  "type": "module",
  "description": "Houou — 自作将棋エンジンの wasm 束ね",
  "version": "0.1.0",
  "license": "Apache-2.0",
  "repository": {
    "type": "git",
    "url": "https://github.com/KenichiroMatsubara/HayateProjects"
  },
  "files": [
    "houou_bg.wasm",
    "houou.js",
    "houou.d.ts"
  ],
  "main": "houou.js",
  "types": "houou.d.ts",
  "sideEffects": [
    "./snippets/*"
  ]
}
JSON

echo
echo "できた: $OUT_DIR"
ls -la "$OUT_DIR"
