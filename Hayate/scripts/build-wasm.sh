#!/usr/bin/env bash
# scripts/build-wasm.sh — hayate-adapter-web を wasm-pack でビルドする
set -euo pipefail

# Source Cargo env so non-interactive shells (npm, VS Code tasks) find cargo/wasm-pack
# shellcheck source=/dev/null
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CRATE_DIR="$ROOT_DIR/crates/platform/web"
OUT_DIR="$ROOT_DIR/wasm-pkgs/pkg"
OUT_DIR_CPU="$ROOT_DIR/wasm-pkgs/pkg-tiny-skia"
OUT_DIR_NULL="$ROOT_DIR/wasm-pkgs/pkg-null"
PKG_GITIGNORE=$'*\n!package.json'
LOCK_FILE="${TMPDIR:-/tmp}/hayate-wasm-build.lock"

# wasm-pack は毎回 package.json を再生成するが、その出力レイアウトはバージョン間で揺れる
# （0.15 では description/repository/files/sideEffects が落ちる）。これが追跡対象の
# package.json に毎回ノイズ差分を生み、IDE のビルド/同期のたびに差分が出る原因だった。
# これらは公開せず社内の `file:` 依存としてのみ消費されるので、ここで正規版を固定して
# wasm-pack の出力を上書きする。`sideEffects` は wasm-bindgen の snippets をバンドラに
# tree-shake されないために必須なので残す。
finalize_pkg() {
  local dir=$1
  printf '%s\n' "$PKG_GITIGNORE" > "$dir/.gitignore"
  cat > "$dir/package.json" <<'JSON'
{
  "name": "hayate-adapter-web",
  "type": "module",
  "description": "Hayate — GPU-native UI substrate",
  "version": "0.1.0",
  "license": "Apache-2.0",
  "repository": {
    "type": "git",
    "url": "https://github.com/KenichiroMatsubara/HayateProjects"
  },
  "files": [
    "hayate_adapter_web_bg.wasm",
    "hayate_adapter_web.js",
    "hayate_adapter_web.d.ts"
  ],
  "main": "hayate_adapter_web.js",
  "types": "hayate_adapter_web.d.ts",
  "sideEffects": [
    "./snippets/*"
  ]
}
JSON
}

# pnpm -r can invoke this script concurrently; serialize wasm-pack on one crate dir.
# flock は Linux/WSL では使えるが Git Bash(Git for Windows)には無いので、
# 無い環境では排他をスキップする(直列実行されるケースでは元々ロック不要)。
if command -v flock >/dev/null 2>&1; then
  exec 9>"$LOCK_FILE"
  if ! flock -n 9; then
    echo "Waiting for another hayate WASM build to finish..."
    flock 9
  fi
fi

BOLD='\033[1m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
RED='\033[0;31m'
RESET='\033[0m'

echo -e "${BOLD}━━━ hayate WASM build ━━━${RESET}"
echo    "  root : $ROOT_DIR"
echo    "  crate: $CRATE_DIR"
echo    "  out  : $OUT_DIR"
echo

# wasm-pack expects LICENSE beside the crate manifest.
cp "$ROOT_DIR/LICENSE" "$CRATE_DIR/LICENSE"

# ── Step 1: cargo check (wasm32) ─────────────────────────────────────────────
echo -e "${CYAN}▶ cargo check (wasm32-unknown-unknown)...${RESET}"
cargo check \
  --manifest-path "$ROOT_DIR/Cargo.toml" \
  -p hayate-core \
  -p hayate-adapter-web \
  --target wasm32-unknown-unknown
echo

# ── Step 2: wasm-pack build ──────────────────────────────────────────────────
echo -e "${CYAN}▶ wasm-pack build --target web...${RESET}"
wasm-pack build "$CRATE_DIR" \
  --target web \
  --out-dir "$OUT_DIR"
finalize_pkg "$OUT_DIR"
echo

# ── Step 3: wasm-pack build (tiny-skia CPU backend) ─────────────────────────
echo -e "${CYAN}▶ wasm-pack build --target web (backend-tiny-skia)...${RESET}"
wasm-pack build "$CRATE_DIR" \
  --target web \
  --out-dir "$OUT_DIR_CPU" \
  -- --no-default-features --features backend-tiny-skia
finalize_pkg "$OUT_DIR_CPU"
echo

# ── Step 4: wasm-pack build (null backend — C3 codec integration tests) ─────
echo -e "${CYAN}▶ wasm-pack build --target web (backend-null)...${RESET}"
wasm-pack build "$CRATE_DIR" \
  --target web \
  --out-dir "$OUT_DIR_NULL" \
  -- --no-default-features --features backend-null
finalize_pkg "$OUT_DIR_NULL"
echo

echo -e "${GREEN}${BOLD}✓ Done!${RESET}"
echo    "  pkg         → wasm-pkgs/pkg/"
echo    "  pkg-tiny-skia → wasm-pkgs/pkg-tiny-skia/"
echo    "  pkg-null    → wasm-pkgs/pkg-null/"
echo    "  consumed by Tsubame renderer-canvas (file: deps in wasm-pkgs/*)"
