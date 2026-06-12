#!/usr/bin/env bash
# Verify hayate-adapter-android stage A on a local emulator or device (issue #195).
#
# Prerequisites:
#   - Android SDK (ANDROID_HOME) and NDK (ANDROID_NDK_HOME)
#   - cargo-apk: cargo install cargo-apk
#   - An emulator running or a USB-debugged device (`adb devices` shows one)
#
# Usage:
#   ./scripts/verify-android-stage-a.sh
#
# Expected result: APK installs, launches, and shows a dark gray-blue clear color
# (CLEAR_COLOR ≈ RGB 26/26/31). Rotate the device or send the app to background
# and return — it should not crash.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

: "${ANDROID_HOME:?Set ANDROID_HOME to your Android SDK path}"
: "${ANDROID_NDK_HOME:?Set ANDROID_NDK_HOME to your Android NDK path}"

NDK_BIN="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin"
export CC_aarch64_linux_android="${NDK_BIN}/aarch64-linux-android21-clang"
export AR_aarch64_linux_android="${NDK_BIN}/llvm-ar"
export RANLIB_aarch64_linux_android="${NDK_BIN}/llvm-ranlib"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="${NDK_BIN}/aarch64-linux-android21-clang"

echo "==> Building APK"
cargo apk build -p hayate-adapter-android --lib

APK="$ROOT/target/debug/apk/hayate-adapter-android.apk"
if [[ ! -f "$APK" ]]; then
  echo "APK not found at $APK" >&2
  exit 1
fi

ADB="$ANDROID_HOME/platform-tools/adb"
if ! "$ADB" get-state >/dev/null 2>&1; then
  echo "No adb device/emulator connected. Start an emulator or plug in a device." >&2
  exit 1
fi

echo "==> Installing APK"
"$ADB" install -r "$APK"

echo "==> Launching com.hayateprojects.hayate.adapter_android_demo"
"$ADB" shell am start -n com.hayateprojects.hayate.adapter_android_demo/android.app.NativeActivity

echo ""
echo "Manual checks (issue #195 acceptance criteria):"
echo "  1. Screen shows dark gray-blue clear color (not black/white crash screen)"
echo "  2. Rotate device or resize window — app keeps running"
echo "  3. Home button → reopen app — surface recreates without crash"
echo ""
echo "Capture evidence with:"
echo "  $ADB logcat -s hayate-adapter-android:* AndroidRuntime:E"
echo "  $ADB exec-out screencap -p > stage-a-screenshot.png"
