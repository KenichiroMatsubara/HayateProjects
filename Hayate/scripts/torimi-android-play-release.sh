#!/usr/bin/env bash
# scripts/torimi-android-play-release.sh — Torimi Android 版を Google Play ストアへリリースするための
# 署名済み AAB を、versionCode 自動採番でビルドする
#
# Google Play は同一 versionCode の再アップロードを拒否するので、Play に上げるたびに
# versionCode を +1 しないといけない（RELEASE-SIGNING.md「2 回目以降を上げるたびに +1」）。
# 一方で app/build.gradle.kts の versionCode をコミットで増やし続けると差分が汚れる。
#
# そこでこのスクリプトは:
#   1. Git 非追跡のカウンタファイル（.torimi-play-versioncode）に「前回使った versionCode」を保存し、
#   2. build.gradle.kts の versionCode を一瞬だけ「前回値 + 1」に書き換え、
#   3. その状態で署名済み AAB（bundleRelease）をビルドし、
#   4. build.gradle.kts を元の値に必ず戻す（成功しても失敗しても・Ctrl-C でも trap で復元）。
#   5. ビルドが成功したときだけカウンタファイルを新しい値へ進める（失敗時は番号を消費しない）。
#
# 結果として git diff は常にクリーンなまま、Play に上げる AAB だけが毎回インクリメントされる。
#
# 使い方:
#   ./scripts/torimi-android-play-release.sh              # 前回値+1 で AAB をビルド
#   ./scripts/torimi-android-play-release.sh --dry-run    # 採番だけ表示してビルドしない（書き換えもしない）
#   ./scripts/torimi-android-play-release.sh --set N      # 次回使う versionCode を N に固定してビルド（ズレ補正用）
#
# 追加引数は build-android.sh 経由で Gradle に転送される:
#   ./scripts/torimi-android-play-release.sh -Psome.flag=1
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ANDROID_APP_DIR="$ROOT_DIR/crates/platform/mobile/android/android-app"
GRADLE_FILE="$ANDROID_APP_DIR/app/build.gradle.kts"
# Git 非追跡（.gitignore 済み）。ここに「前回ビルドで使った versionCode」を 1 行で持つ。
COUNTER_FILE="$ANDROID_APP_DIR/.torimi-play-versioncode"

BOLD='\033[1m'; GREEN='\033[0;32m'; CYAN='\033[0;36m'; YELLOW='\033[0;33m'; RED='\033[0;31m'; RESET='\033[0m'

die() { echo -e "${RED}✗ $*${RESET}" >&2; exit 1; }

# ── 引数パース ────────────────────────────────────────────────────────────────
DRY_RUN=0
SET_VALUE=""
GRADLE_EXTRA=()
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) DRY_RUN=1; shift ;;
    --set) SET_VALUE="${2:-}"; [ -n "$SET_VALUE" ] || die "--set には数値が要ります"; shift 2 ;;
    -h|--help) sed -n '2,26p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) GRADLE_EXTRA+=("$1"); shift ;;
  esac
done

[ -f "$GRADLE_FILE" ] || die "build.gradle.kts が見つかりません: $GRADLE_FILE"

# ── 現在の versionCode（コミット済みベースライン）を読む ──────────────────────
BASELINE="$(grep -E '^[[:space:]]*versionCode[[:space:]]*=' "$GRADLE_FILE" | grep -oE '[0-9]+' | head -1)"
[ -n "$BASELINE" ] || die "build.gradle.kts の versionCode 行を読み取れませんでした"

# ── 次に使う versionCode を決める ────────────────────────────────────────────
# 前回値はカウンタファイル、無ければ gradle のベースラインを「前回値」とみなす。
# gradle 側を手で上げていた場合に取りこぼさないよう、両者の大きい方 +1 を採用する。
if [ -n "$SET_VALUE" ]; then
  echo "$SET_VALUE" | grep -qE '^[0-9]+$' || die "--set の値が数値ではありません: $SET_VALUE"
  NEW="$SET_VALUE"
else
  if [ -f "$COUNTER_FILE" ]; then
    LAST="$(tr -dc '0-9' < "$COUNTER_FILE")"
  else
    LAST=""
  fi
  LAST="${LAST:-$BASELINE}"
  HIGH="$BASELINE"; [ "$LAST" -gt "$HIGH" ] && HIGH="$LAST"
  NEW=$((HIGH + 1))
fi

echo -e "${BOLD}━━━ Torimi Android → Google Play リリースビルド (versionCode 自動採番) ━━━${RESET}"
echo    "  gradle baseline : $BASELINE"
echo    "  counter file    : $COUNTER_FILE ($( [ -f "$COUNTER_FILE" ] && cat "$COUNTER_FILE" || echo '未作成' ))"
echo -e "  次回 versionCode: ${GREEN}${BOLD}$NEW${RESET}"
echo

if [ "$DRY_RUN" -eq 1 ]; then
  echo -e "${YELLOW}--dry-run: build.gradle.kts は書き換えずに終了します。${RESET}"
  exit 0
fi

# ── build.gradle.kts をバックアップし、EXIT で必ず原状復帰させる ─────────────
BACKUP="$(mktemp "${TMPDIR:-/tmp}/build.gradle.kts.XXXXXX")"
cp "$GRADLE_FILE" "$BACKUP"
restore() {
  # 成功・失敗・Ctrl-C いずれでも元の versionCode（＝コミット済みの値）へ戻す。
  cp -f "$BACKUP" "$GRADLE_FILE"
  rm -f "$BACKUP"
}
trap restore EXIT

# versionCode 行だけを NEW に差し替える（インデント/前後は保持）。
sed -i -E "s/^([[:space:]]*versionCode[[:space:]]*=[[:space:]]*)[0-9]+/\1${NEW}/" "$GRADLE_FILE"
# 書き換えが効いたか検証（効かなかったまま古い code でビルド→Play 拒否、を防ぐ）。
GOT="$(grep -E '^[[:space:]]*versionCode[[:space:]]*=' "$GRADLE_FILE" | grep -oE '[0-9]+' | head -1)"
[ "$GOT" = "$NEW" ] || die "versionCode の書き換えに失敗しました（期待 $NEW / 実際 $GOT）"

echo -e "${CYAN}▶ versionCode=$NEW で bundleRelease をビルドします${RESET}"
echo
# 署名済み AAB を作る。JDK/SDK/cargo env の解決は build-android.sh に委譲。
bash "$SCRIPT_DIR/build-android.sh" bundleRelease "${GRADLE_EXTRA[@]}"

AAB="$ANDROID_APP_DIR/app/build/outputs/bundle/release/app-release.aab"

# ── 署名検証ガード ────────────────────────────────────────────────────────────
# build.gradle.kts は署名情報（hayate.release.* 4 つ）が揃わないと bundleRelease を
# 「未署名 AAB」として黙って成功させる。その未署名 AAB は Play にアップして初めて拒否され、
# しかもここまで来ると下でカウンタが +1 進むため versionCode だけ無駄に消費してズレる。
# それを防ぐため、カウンタを進める前に AAB が実際に署名されているかを検証し、
# 未署名なら die → EXIT trap が gradle を復元し、カウンタは進めない（＝番号を消費しない）。
[ -f "$AAB" ] || die "AAB が生成されていません: $AAB（build-android.sh の出力を確認）"
# 署名済み AAB は jarsigner 署名（v1）の証跡として META-INF に *.RSA/*.DSA/*.EC を持つ。
if ! unzip -l "$AAB" 2>/dev/null | grep -qiE 'META-INF/.*\.(RSA|DSA|EC)$'; then
  die "生成された AAB が未署名です: $AAB
  → 署名情報（hayate.release.storeFile/storePassword/keyAlias/keyPassword）が
    ~/.gradle/gradle.properties か環境変数に揃っているか確認してください（RELEASE-SIGNING.md）。
  → カウンタ($COUNTER_FILE)は進めていないので、設定を直して再実行すれば同じ versionCode=$NEW で採番されます。"
fi
echo -e "${GREEN}✓ 署名を確認しました（署名済み AAB）${RESET}"

# ここに来たらビルド成功かつ署名済み。カウンタファイルを新しい値へ進める（失敗時は
# trap→restore だけでここは実行されず、番号は消費されない）。
echo "$NEW" > "$COUNTER_FILE"
echo
echo -e "${GREEN}${BOLD}✓ Done! versionCode=$NEW の署名済み AAB を作成しました${RESET}"
echo    "  AAB      : $AAB"
echo    "  counter  : $COUNTER_FILE → $NEW（次回は $((NEW + 1)) を採番）"
echo    "  gradle   : versionCode は $BASELINE に復元済み（git diff はクリーン）"
echo
echo -e "  次の手順: この AAB を Play Console → 内部テスト → 新しいリリース にアップロード"
echo    "           （RELEASE-SIGNING.md 手順 4）"
