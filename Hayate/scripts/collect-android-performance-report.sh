#!/usr/bin/env bash
# Captures one profileable benchmark run: fixed HayatePerf summaries plus a Perfetto trace.
set -euo pipefail

readonly PACKAGE_NAME="com.hayateprojects.torimi.benchmark"
readonly PERFETTO_DURATION_SECONDS="20"
readonly REMOTE_TRACE_PATH="/data/misc/perfetto-traces/hayate-performance.perfetto-trace"
readonly SUMMARY_TAG="HayatePerf"

OUTPUT_DIR="${1:-android-performance-report}"
mkdir -p "$OUTPUT_DIR"

adb logcat -c
adb shell am force-stop "$PACKAGE_NAME"
adb shell am start -n "$PACKAGE_NAME/com.hayateprojects.hayate.adapter_android_demo.MainActivity" >/dev/null
adb shell perfetto --out "$REMOTE_TRACE_PATH" \
  --txt -c - <<EOF
duration_ms: $((PERFETTO_DURATION_SECONDS * 1000))
buffers: { size_kb: 8192 fill_policy: RING_BUFFER }
data_sources: {
  config {
    name: "linux.ftrace"
    ftrace_config {
      ftrace_events: "sched/sched_switch"
      ftrace_events: "ftrace/print"
      atrace_categories: "gfx"
      atrace_categories: "view"
      atrace_apps: "$PACKAGE_NAME"
    }
  }
}
EOF
adb pull "$REMOTE_TRACE_PATH" "$OUTPUT_DIR/hayate.perfetto-trace"
adb logcat -d -s "$SUMMARY_TAG" > "$OUTPUT_DIR/hayate-adb-summary.txt"
adb shell dumpsys gfxinfo "$PACKAGE_NAME" framestats > "$OUTPUT_DIR/hayate-gfx-framestats.txt"
