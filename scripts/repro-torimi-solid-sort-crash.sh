#!/usr/bin/env bash
set -euo pipefail

serial="${1:-}"
if [[ -z "$serial" ]]; then
  serial="$(adb devices | awk 'NR > 1 && $2 == "device" { print $1; exit }')"
fi

chip="${2:-name}"
case "$chip" in
  name) chip_x=341 ;;
  priority) chip_x=527 ;;
  *)
    echo "unknown sort chip '$chip' (expected name or priority)" >&2
    exit 2
    ;;
esac
if [[ -z "$serial" ]]; then
  echo "no connected Android device" >&2
  exit 2
fi

package="com.hayateprojects.torimi.debug"
setup_activity="com.hayateprojects.hayate.adapter_android_demo.DevServerSetupActivity"
main_activity="com.hayateprojects.hayate.adapter_android_demo.MainActivity"
marker="torimi-solid-sort-$(date +%s)"

focused_app() {
  adb -s "$serial" shell dumpsys window \
    | sed -n 's/.*mFocusedApp=ActivityRecord{[^ ]* [^ ]* \([^ ]*\).*/\1/p' \
    | head -n 1
}

adb -s "$serial" shell input keyevent KEYCODE_WAKEUP
adb -s "$serial" shell wm dismiss-keyguard
adb -s "$serial" shell am force-stop "$package"
adb -s "$serial" shell monkey -p "$package" -c android.intent.category.LAUNCHER 1 >/dev/null
sleep 1

focused="$(focused_app)"
if [[ "$focused" != *"$setup_activity"* ]]; then
  echo "launcher did not reach setup activity: $focused" >&2
  exit 2
fi

# The first native launcher button is "TODO (SOLID)".
adb -s "$serial" shell input tap 540 287
sleep 5

focused="$(focused_app)"
if [[ "$focused" != *"$main_activity"* ]]; then
  echo "Solid demo did not reach MainActivity: $focused" >&2
  exit 2
fi

pid_before="$(adb -s "$serial" shell pidof "$package")"
adb -s "$serial" shell log -p i -t CODEX_REPRO "$marker"

# 1080x2400 A101OP portrait layout: center of the requested visible sort chip.
adb -s "$serial" shell input tap "$chip_x" 2100
sleep 2

pid_after="$(adb -s "$serial" shell pidof "$package" || true)"
focused="$(focused_app)"

if [[ -z "$pid_after" || "$pid_after" != "$pid_before" || "$focused" != *"$main_activity"* ]]; then
  echo "RED: tapping $chip terminated Torimi (before=$pid_before after=${pid_after:-none})" >&2
  adb -s "$serial" logcat -d -v brief -t 800 \
    | sed -n "/CODEX_REPRO.*$marker/,\$p" \
    | grep -E -i 'CODEX_REPRO|torimi|hayate|hermes|fatal|exception|crash|abort|libc' \
    | head -n 80 >&2
  exit 1
fi

echo "GREEN: tapping $chip kept Torimi alive (pid=$pid_after)"
