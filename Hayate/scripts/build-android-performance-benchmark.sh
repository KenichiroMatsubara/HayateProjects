#!/usr/bin/env bash
# Builds the release-optimised, profileable Android benchmark variant from ADR-0156.
set -euo pipefail

readonly BENCHMARK_TASK="assembleBenchmark"
readonly BENCHMARK_PROPERTY="-Pbenchmark"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec bash "$SCRIPT_DIR/build-android.sh" "$BENCHMARK_TASK" "$BENCHMARK_PROPERTY" "$@"
