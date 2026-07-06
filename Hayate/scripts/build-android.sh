#!/usr/bin/env bash
# scripts/build-android.sh — android-app(Gradle) を叩く薄いラッパー（ADR-0094 / ADR-0112）
#
# Android の正規ビルドは crates/platform/mobile/android/android-app の Gradle プロジェクト
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

# Android Studio GUI を開かずに叩けるよう、JDK と SDK を自前で解決する。
# 既に環境変数 / PATH で見えていれば尊重し、無ければ既知の既定位置にフォールバックする。

# JAVA_HOME: 未設定かつ java が PATH に無ければ Android Studio 同梱 JBR を使う。
if [ -z "${JAVA_HOME:-}" ] && ! command -v java >/dev/null 2>&1; then
  for jbr in \
    "/c/Program Files/Android/Android Studio/jbr" \
    "${LOCALAPPDATA:-}/Programs/Android Studio/jbr" \
    "$HOME/AppData/Local/Programs/Android Studio/jbr" \
    "$HOME/android-studio/jbr" \
    "/opt/android-studio/jbr" \
    "/usr/local/android-studio/jbr"; do
    if [ -x "$jbr/bin/java" ] || [ -x "$jbr/bin/java.exe" ]; then
      export JAVA_HOME="$jbr"
      break
    fi
  done
fi

# ANDROID_HOME: 未設定なら local.properties(sdk.dir) → 既定 SDK 位置 の順に解決する。
if [ -z "${ANDROID_HOME:-}" ] && [ -z "${ANDROID_SDK_ROOT:-}" ]; then
  for sdk in \
    "${LOCALAPPDATA:-}/Android/Sdk" \
    "$HOME/AppData/Local/Android/Sdk" \
    "$HOME/Android/Sdk"; do
    if [ -d "$sdk" ]; then
      export ANDROID_HOME="$sdk"
      export ANDROID_SDK_ROOT="$sdk"
      break
    fi
  done
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ANDROID_DIR="$ROOT_DIR/crates/platform/mobile/android/android-app"

# cargo の出力先はワークスペース共有の `$ROOT_DIR/target`。かつてはここで export しないと
# rust-android-gradle が既定の `<cargo module>/target` を見て .so を取りこぼし、jniLibs 空の
# 起動即クラッシュ APK が「成功」扱いで出来上がっていた。現在は app/build.gradle.kts が
# `cargo.targetDirectory` をワークスペース target に固定し、さらに verifyRustJniLib タスクが
# .so の欠落・stale を検出してビルドを失敗させるため、素の `./gradlew` でも安全。
# この export は他ツールとの整合を保つための念押し（プラグインの解決順は
# rust.cargoTargetDir → env CARGO_TARGET_DIR → cargo.targetDirectory → 既定）。
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT_DIR/target}"

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
echo    "  APK 出力 → crates/platform/mobile/android/android-app/app/build/outputs/apk/"
