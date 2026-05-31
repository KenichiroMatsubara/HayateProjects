#!/usr/bin/env bash
# scripts/build-wasm.sh — hayate-adapter-web を wasm-pack でビルドする
set -euo pipefail

# Source Cargo env so non-interactive shells (npm, VS Code tasks) find cargo/wasm-pack
# shellcheck source=/dev/null
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CRATE_DIR="$ROOT_DIR/crates/adapters/web"
OUT_DIR="$ROOT_DIR/examples/web-demo/pkg"

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
echo

echo -e "${GREEN}${BOLD}✓ Done!${RESET}  pkg → examples/web-demo/pkg/"
echo    "  Next : npm run serve"
