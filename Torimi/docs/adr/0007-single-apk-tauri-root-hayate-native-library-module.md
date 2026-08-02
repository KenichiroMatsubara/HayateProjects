# 1 APK 構成は Tauri をルート・Hayate ネイティブを library モジュールに分け、Gradle プロジェクトを Torimi 配下へ移し、Vulkan 必須宣言を実行時ゲートへ置き換える

status: accepted

Date: 2026-08-02

## Context

ADR-0006 で Torimi Shell を Tauri 2 に載せると決めた結果、Android の実体をどう組むかを決める必要が
ある。既存の出荷 APK（`com.hayateprojects.torimi`）は手入れされた Gradle プロジェクトで、
`rust-android-gradle` による cdylib ビルド・Hermes AAR・GameActivity・ML Kit code-scanner・
release 署名構成を抱えている。一方 Tauri は Android について**独立した Gradle ルートプロジェクト
一式**（`settings.gradle`・`buildSrc` の Rust プラグイン・wrapper・`app` モジュール）を生成して
所有するモデルを取る。つまり衝突は「ルートプロジェクトが 2 つある」ことであって、API の衝突では
ない。

加えて現状は、Torimi の出荷 APK が `Hayate/crates/platform/mobile/android/android-app/`（Kotlin
package は `adapter_android_demo`）に住んでおり、Torimi を独立 context とする CONTEXT-MAP と
噛み合っていない。

## Decision

- **Tauri の app モジュールをルート APK とし、既存の app モジュールを Android library モジュールへ
  降格する。** GameActivity・Hermes・Hayate の cdylib はその library に閉じ込め、Tauri 側は
  それを依存として取り込む。モジュール境界がそのまま 2 系統の切れ目になるので、Tauri の `buildSrc`
  と既存の `rust-android-gradle` が同じモジュール内で絡み合うことがない。
- **cdylib は 2 つのまま保ち、1 つに統合しない。** `ndk-context` が保持する JavaVM / Context の
  グローバルは shared object ごとのインスタンスであり、Tauri 側と Hayate 側を同一 `.so` に
  マージすると両者が同じグローバルを奪い合う。分けておけば同一 APK 内に共存できる。
- **Activity は 3 枚**（Tauri Shell / Bundle Preview の GameActivity / 既存の入力画面の後継）。
  往復は明示 Intent で行う。これは `DevServerSetupActivity` → `MainActivity` として既に動いている
  構図の反復である。
- **Gradle プロジェクトは `Torimi/` 配下へ移す。** ただし Rust の Android leaf crate は Hayate に
  残す（ADR-0117 の三層モデルで leaf は Hayate 所有）。移動にあたり、現在 Gradle 側から Hayate の
  crate へ向いている逆流を断つ:
  - vendored Hermes の `.so` を Gradle の `jniLibs` から **Hayate crate の `third_party/` 配下へ移設**し、
    `build.rs` は crate 内で完結させる。Gradle は `jniLibs.srcDirs` でそこを指す（Torimi → Hayate の正方向）。
  - `tests/apk_packaging.rs` の packaging 契約テストは **Torimi 側へ移す**。packaging は Torimi の
    所有物になるため、Hayate の crate が Torimi 配下を読みに行く形（依存方向違反）を残さない。
- **`android.hardware.vulkan.level` の `required` を false にし、実行時ゲートに置き換える。**
  端末が Vulkan を満たさない場合は Bundle Preview の入口を無効化し、理由を表示する。Live Preview は
  全端末で使えるようにする。renderer fallback の実装は前提としない — 必要なのは fallback ではなく
  明示的な拒否である。`required="true"` は「Vulkan の無い端末で GameActivity が黙って落ちる」ことを
  インストールフィルタで隠していただけであり、実行時に判定すれば宣言に依存する理由がなくなる。
- **`abiFilters = arm64-v8a` は据え置く。** Hayate の cdylib と Hermes の都合であり、Vulkan 宣言とは
  別軸。実質的な影響は小さい。

## Considered Options

- **既存 Gradle をルートに保ち Tauri を手で埋め込む。** Tauri CLI の開発ループとプラグイン配線を
  捨てることになり、Tauri を採る意味を最も損なう。
- **Tauri 生成物をルートにして既存 app モジュールへ全部混ぜる。** 2 系統の Rust ビルド機構が同じ
  モジュールに恒久的に同居する。
- **Rust の Android leaf crate ごと Torimi へ移す。** パスは素直になるが、ADR-0117 の三層モデル
  （契約は Core・leaf は Hayate 所有）に正面から反する。
- **flavor / 別 APK に分けて Vulkan 要件を出し分ける。** Play 上の管理が二重化する。

## Consequences

- `applicationId`・`versionCode`・release 署名構成は Tauri 側 app モジュールへ引き継ぐ必要がある
  （library に降格したモジュールは `applicationId` を持てない）。Play の継続更新に直結するため、
  検証段階では別 id のデバッグビルドを使い、出荷 id の引き継ぎは最後に行う。
- 移動の影響範囲は `scripts/build-android.sh`・`scripts/torimi-android-play-release.sh`・
  `RELEASE-SIGNING.md`・`third_party/NOTICE.md`・ADR-0094・`docs/spec/09-platform-accessibility.md`
  に及ぶ。
- Kotlin package 名 `com.hayateprojects.hayate.adapter_android_demo` は JNI のシンボル名に焼き
  込まれている（`Java_com_hayateprojects_hayate_adapter_1android_1demo_*`）。ディレクトリ移動と
  package 改名は別作業であり、同時に行わない。
