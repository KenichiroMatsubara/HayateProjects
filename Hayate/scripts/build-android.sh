#!/usr/bin/env bash
# scripts/build-android.sh — android-app(Gradle) を叩く薄いラッパー（ADR-0094 / ADR-0112）
#
# Android の正規ビルドは crates/adapters/android/android-app の Gradle プロジェクト
# （GameActivity + rust-android-gradle）。cargo-apk 経路（verify-android-stage-a.sh）は
# 旧 NativeActivity 用で、現行の Tsubame/Hermes 同梱ビルドは Gradle 側に集約されている。
#
# 使い方:
#   ./scripts/build-android.sh assembleDebug          # デバッグ APK
#   ./scripts/build-android.sh assembleRelease        # リリース APK
#   ./scripts/build-android.sh installDebug           # 接続中の端末/エミュレータへ導入
#   ./scripts/build-android.sh clean                  # Gradle clean
#   ./scripts/build-android.sh installDebug -Pnativedemo  # Hayate 単体デモ（Tsubame 非同梱）
#
# 渡した引数はそのまま Gradle に転送する。Gradle プロパティ（-P…）や追加タスクも併用可。
#
# 前提:
#   - Android SDK / NDK（local.properties か ANDROID_HOME / sdk.dir）
#   - 後述の優先順で見つかる Gradle 実行系（ラッパー or PATH 上の gradle）
set -euo pipefail

# 非対話シェル（npm / VS Code タスク）でも cargo/rustup を引けるよう Cargo env を読む。
# rust-android-gradle が cargo を呼ぶため。
# shellcheck source=/dev/null
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ANDROID_DIR="$ROOT_DIR/crates/adapters/android/android-app"

BOLD='\033[1m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
RED='\033[0;31m'
RESET='\033[0m'

# 既定タスク（引数なしで実行されたとき）。
TASKS=("$@")
if [ ${#TASKS[@]} -eq 0 ]; then
  TASKS=("assembleDebug")
fi

# Gradle 実行系を解決する。優先順:
#   1. リポジトリ同梱のラッパー（./gradlew / gradlew.bat）— あれば最優先（バージョン固定）
#   2. PATH 上の gradle
# どちらも無ければラッパー生成方法を案内して終了する。
# Windows の Git Bash でも ./gradlew（POSIX sh スクリプト）はそのまま実行できる。
resolve_gradle() {
  if [ -x "$ANDROID_DIR/gradlew" ] || [ -f "$ANDROID_DIR/gradlew" ]; then
    GRADLE=("bash" "$ANDROID_DIR/gradlew")
    return 0
  fi
  if command -v gradle >/dev/null 2>&1; then
    GRADLE=("gradle")
    return 0
  fi
  return 1
}

if ! resolve_gradle; then
  echo -e "${RED}Gradle が見つかりません。${RESET}" >&2
  echo "  android-app には Gradle ラッパーが未コミットで、PATH 上にも gradle がありません。" >&2
  echo "  いずれかで用意してください（gradle-wrapper.properties は Gradle 8.13 固定）:" >&2
  echo "    - Android Studio で android-app を開く（同梱 Gradle を使用）" >&2
  echo "    - もしくは gradle 8.13 を導入後、android-app で 'gradle wrapper' を実行してラッパーを生成" >&2
  exit 1
fi

echo -e "${BOLD}━━━ hayate Android build ━━━${RESET}"
echo    "  root   : $ROOT_DIR"
echo    "  project: $ANDROID_DIR"
echo    "  gradle : ${GRADLE[*]}"
echo    "  tasks  : ${TASKS[*]}"
echo

echo -e "${CYAN}▶ ${GRADLE[*]} ${TASKS[*]}${RESET}"
( cd "$ANDROID_DIR" && "${GRADLE[@]}" "${TASKS[@]}" )

echo
echo -e "${GREEN}${BOLD}✓ Done!${RESET}"
echo    "  APK 出力 → crates/adapters/android/android-app/app/build/outputs/apk/"
