# 依存ライブラリ・ツール バージョン比較（更新前スナップショット）

取得日: **2026-07-22**

この表は、一括更新を行う直前の比較結果を保存したスナップショットです。Git 管理下の `package.json`、`Cargo.toml`、Gradle Kotlin DSL、Gradle wrapper に直接宣言された外部依存をパッケージ名ごとに統合しました。`workspace:*`、ローカル `path`、生成済み WASM package は除外し、リポジトリ内にコピーされている Rust crate は `(vendored)` として含めています。`lock / bundled` は更新前の全 lockfileまたは同梱ソースで見つかった版です。

一括更新後の互換性検証で、最新メジャーのままでは通常・Wasm・Androidの全ビルドを同時に成立させられない依存だけを、下表の互換版へ戻しました。現在の宣言値と lockfile は各 manifest およびルート `pnpm-lock.yaml` を正本とし、この文書の全件比較表は更新前の差分確認用として残します。

## 更新後の互換ピン

| ライブラリ / ツール | 採用版 | 最新安定版 | 理由 |
|---|---:|---:|---|
| TypeScript | `6.0.3` | `7.0.2` | `tsup 8.5.1` / `rollup-plugin-dts 6.1.1` が TypeScript 7 compiler API と未互換。pnpm override で optional peer も6系へ統一 |
| Gradle | `8.14.5` | `9.6.1` | `rust-android-gradle 0.9.6` が Gradle 9で削除された `setFileMode(Integer)` を使用 |
| Android Gradle Plugin | `8.13.2` | `9.3.0` | `rust-android-gradle 0.9.6` の legacy Android DSL と互換な最新系列 |
| Kotlin Gradle Plugin | `2.3.21` | `2.4.10` | AGP 8.13.2 が公式対応する Kotlin 2.3系列の最新パッチ |
| AndroidX Core | `1.17.0` | `1.19.0` | 1.19.0 は AGP 9.1 / compileSdk 37必須。1.17.0 は AGP 8.13 / compileSdk 36互換 |
| wgpu | `29.0.3` | `30.0.0` | Vello 0.9.0 の公式依存版。wgpu 30向けの独自API追従を撤去 |
| naga | `29.0.4` | `30.0.0` | Vello 0.9.0 / wgpu 29系列に揃えるため |
| jni（Hayate直接依存） | `0.21.1` | `0.22.4` | Android JNI実装が 0.21 APIを使用。0.22移行は別途の破壊的API移行が必要 |

上記の状態で `pnpm build`、`pnpm test`、`cargo test --workspace --all-targets`、Wasm 3構成（Vello / tiny-skia / null）、Android `assembleDebug` を完走しています。
最終の `pnpm outdated -r --format json` は、意図して固定した TypeScript `6.0.3 → 7.0.2` だけを報告し、それ以外の npm 直接依存には更新候補がありません。

## 集計

- 対象: **108** 件（npm/pnpm 25、Rust crates 73、Gradle/Android 10）
- 最新: **47** 件
- 更新あり: **61** 件
- 要確認・取得失敗・先行版: **0** 件

## npm / pnpm

| ライブラリ / ツール | 宣言バージョン | 現在（lock / bundled） | 最新安定版 | 判定 | 利用領域 |
|---|---:|---:|---:|---|---|
| [@babel/core](https://www.npmjs.com/package/@babel/core) | `^8.0.1` | `8.0.1` | `8.0.1` | 最新 | Torimi, Tsubame |
| [@babel/preset-env](https://www.npmjs.com/package/@babel/preset-env) | `^8.0.2` | `8.0.2` | `8.0.2` | 最新 | Torimi, Tsubame |
| [@changesets/cli](https://www.npmjs.com/package/@changesets/cli) | `^2.29.7` | `2.31.0` | `2.31.1` | 更新あり | root |
| [@cloudflare/vitest-pool-workers](https://www.npmjs.com/package/@cloudflare/vitest-pool-workers) | `^0.12.21` | `0.12.21` | `0.18.7` | 更新あり | Torimi |
| [@cloudflare/workers-types](https://www.npmjs.com/package/@cloudflare/workers-types) | `^5.20260705.1` | `5.20260705.1` | `5.20260722.1` | 更新あり | Torimi, fonts |
| [@playwright/test](https://www.npmjs.com/package/@playwright/test) | `^1.61.0` | `1.61.0` | `1.61.1` | 更新あり | Tsubame |
| [@types/babel__core](https://www.npmjs.com/package/@types/babel__core) | `^7.20.5` | `7.20.5` | `7.20.5` | 最新 | Torimi |
| [@types/node](https://www.npmjs.com/package/@types/node) | `^22.15.0`<br>`^22.19.0` | `22.19.19` | `26.1.1` | 更新あり | Torimi, Tsubame |
| [@types/react](https://www.npmjs.com/package/@types/react) | `^19.1.0` | `19.2.17` | `19.2.17` | 最新 | Tsubame |
| [@types/react-reconciler](https://www.npmjs.com/package/@types/react-reconciler) | `^0.32.0` | `0.32.3` | `0.33.0` | 更新あり | Tsubame |
| [ajv](https://www.npmjs.com/package/ajv) | `^8.17.1` | `8.18.0` | `8.20.0` | 更新あり | Hayate |
| [happy-dom](https://www.npmjs.com/package/happy-dom) | `^17.4.4`<br>`^17.6.3` | `17.6.3` | `20.11.0` | 更新あり | Hayate, Tsubame |
| [jsqr](https://www.npmjs.com/package/jsqr) | `^1.4.0` | `1.4.0` | `1.4.0` | 最新 | Torimi |
| [pnpm](https://www.npmjs.com/package/pnpm) | `11.8.0` | `11.8.0` | `11.15.1` | 更新あり | Tsubame, root |
| [react](https://www.npmjs.com/package/react) | `^19.1.0` | `19.2.7` | `19.2.8` | 更新あり | Tsubame |
| [react-reconciler](https://www.npmjs.com/package/react-reconciler) | `^0.32.0` | `0.32.0` | `0.33.0` | 更新あり | Tsubame |
| [rimraf](https://www.npmjs.com/package/rimraf) | `^6.1.3` | `6.1.3` | `6.1.3` | 最新 | Hayate, Torimi, Tsubame |
| [solid-js](https://www.npmjs.com/package/solid-js) | `^1.9.0`<br>`^1.9.13` | `1.9.13` | `1.9.14` | 更新あり | Torimi, Tsubame |
| [tsup](https://www.npmjs.com/package/tsup) | `^8.5.1` | `8.5.1` | `8.5.1` | 最新 | Torimi, Tsubame, root |
| [typescript](https://www.npmjs.com/package/typescript) | `^6.0.3` | `6.0.3` | `7.0.2` | 更新あり | fonts, root |
| [vite](https://www.npmjs.com/package/vite) | `^8.0.0`<br>`^8.0.14` | `6.4.2`<br>`8.0.14` | `8.1.5` | 更新あり | Torimi, Tsubame |
| [vite-plugin-solid](https://www.npmjs.com/package/vite-plugin-solid) | `^2.11.0`<br>`^2.11.12` | `2.11.12` | `2.11.13` | 更新あり | Torimi, Tsubame |
| [vitest](https://www.npmjs.com/package/vitest) | `^3.2.0`<br>`^3.2.6` | `3.2.6` | `4.1.10` | 更新あり | Hayate, Torimi, Tsubame |
| [wrangler](https://www.npmjs.com/package/wrangler) | `^4.107.0`<br>`^4.110.0` | `4.107.0` | `4.113.0` | 更新あり | Torimi, fonts |
| [yaml](https://www.npmjs.com/package/yaml) | `^2.9.0` | `2.9.0` | `2.9.0` | 最新 | Torimi |

## Rust crates

> `vendored` は crates.io の通常更新ではなく、vendoring 元の更新・ローカルパッチとの整合確認が必要です。ロック欄に複数版がある場合は、複数の lockfile または依存グラフで同名 crate の複数版が解決されています。

| ライブラリ / ツール | 宣言バージョン | 現在（lock / bundled） | 最新安定版 | 判定 | 利用領域 |
|---|---:|---:|---:|---|---|
| [accesskit](https://crates.io/crates/accesskit) | `0.24.0` | `0.24.0`<br>`0.24.1` | `0.24.1` | 複数版（更新あり） | Hayate |
| [android-activity](https://crates.io/crates/android-activity) | `0.6` | `0.6.1` | `0.6.1` | 最新 | Hayate |
| [android_logger](https://crates.io/crates/android_logger) | `0.14` | `0.14.1` | `0.15.1` | 更新あり | Hayate |
| [anyhow](https://crates.io/crates/anyhow) | `1` | `1.0.102`<br>`1.0.103` | `1.0.104` | 更新あり | Hayate |
| [arrayvec](https://crates.io/crates/arrayvec) | `0.7` | `0.7.6`<br>`0.7.7` | `0.7.8` | 更新あり | Hayate |
| [bytemuck](https://crates.io/crates/bytemuck) | `1.13.1`<br>`1.25`<br>`1.25.0` | `1.25.0` | `1.25.2` | 更新あり | Hayate |
| [console_error_panic_hook](https://crates.io/crates/console_error_panic_hook) | `0.1` | `0.1.7` | `0.1.7` | 最新 | Hayabusa, Hayate |
| [console_log](https://crates.io/crates/console_log) | `1` | `1.0.0` | `1.1.0` | 更新あり | Hayate |
| [core_maths](https://crates.io/crates/core_maths) | `0.1`<br>`0.1.1` | `0.1.1` | `0.1.1` | 最新 | Hayate |
| [cssparser](https://crates.io/crates/cssparser) | `0.37.0` | `0.37.0` | `0.37.0` | 最新 | Hayate |
| [cxx](https://crates.io/crates/cxx) | `1` | `1.0.194` | `1.0.198` | 更新あり | Hayate |
| [cxx-build](https://crates.io/crates/cxx-build) | `1` | `1.0.194` | `1.0.198` | 更新あり | Hayate |
| [document-features](https://crates.io/crates/document-features) | `0.2.7` | `0.2.12` | `0.2.12` | 最新 | Hayate |
| [env_logger](https://crates.io/crates/env_logger) | `0.11` | `0.11.11` | `0.11.11` | 最新 | Hayate |
| [fontique](https://crates.io/crates/fontique) (vendored) | `0.9.0`<br>`vendored 0.9.0` | `0.9.0` | `0.11.0` | 更新あり | Hayate |
| [futures-intrusive](https://crates.io/crates/futures-intrusive) | `0.5.0` | `0.5.0` | `0.5.0` | 最新 | Hayate |
| [grid](https://crates.io/crates/grid) | `1.0.0` | `1.0.1` | `1.0.1` | 最新 | Hayate |
| [guillotiere](https://crates.io/crates/guillotiere) | `0.7.0` | `0.7.0` | `0.7.0` | 最新 | Hayate |
| [harfrust](https://crates.io/crates/harfrust) | `0.6.0` | `0.6.2` | `0.12.0` | 更新あり | Hayate |
| [hashbrown](https://crates.io/crates/hashbrown) | `0.17.0` | `0.15.5`<br>`0.16.1`<br>`0.17.1` | `0.17.1` | 複数版（更新あり） | Hayate |
| [icu_normalizer](https://crates.io/crates/icu_normalizer) | `2.1.1` | `2.2.0` | `2.2.0` | 最新 | Hayate |
| [icu_properties](https://crates.io/crates/icu_properties) | `2.1.2` | `2.2.0` | `2.2.0` | 最新 | Hayate |
| [icu_segmenter](https://crates.io/crates/icu_segmenter) | `2.1.2` | `2.2.0` | `2.2.0` | 最新 | Hayate |
| [image](https://crates.io/crates/image) | `0.25` | `0.25.10` | `0.25.10` | 最新 | Hayate |
| [imbl](https://crates.io/crates/imbl) | `6` | `6.1.0` | `7.0.1` | 更新あり | Hayate |
| [jni](https://crates.io/crates/jni) | `0.21` | `0.21.1`<br>`0.22.4` | `0.22.4` | 複数版（更新あり） | Hayate |
| [js-sys](https://crates.io/crates/js-sys) | `0.3` | `0.3.103`<br>`0.3.99` | `0.3.103` | 複数版（更新あり） | Hayate |
| [kurbo](https://crates.io/crates/kurbo) | `0.13.0` | `0.13.1` | `0.13.1` | 最新 | Hayate |
| [linebender_resource_handle](https://crates.io/crates/linebender_resource_handle) | `0.1.1` | `0.1.1` | `0.1.1` | 最新 | Hayate |
| [log](https://crates.io/crates/log) | `0.4`<br>`0.4.29` | `0.4.30`<br>`0.4.33` | `0.4.33` | 複数版（更新あり） | Hayate |
| [memmap2](https://crates.io/crates/memmap2) | `0.9.10` | `0.9.10`<br>`0.9.11` | `0.9.11` | 複数版（更新あり） | Hayate |
| [naga](https://crates.io/crates/naga) | `29.0.3` | `29.0.3` | `30.0.0` | 更新あり | Hayate |
| [ndk](https://crates.io/crates/ndk) | `0.9` | `0.9.0` | `0.9.0` | 最新 | Hayate |
| [ndk-context](https://crates.io/crates/ndk-context) | `0.1` | `0.1.1` | `0.1.1` | 最新 | Hayate |
| [ndk-sys](https://crates.io/crates/ndk-sys) | `0.6` | `0.6.0+11769913` | `0.6.0+11769913` | 最新 | Hayate |
| [objc2](https://crates.io/crates/objc2) | `0.6.4` | `0.5.2`<br>`0.6.4` | `0.6.4` | 複数版（更新あり） | Hayate |
| [objc2-core-foundation](https://crates.io/crates/objc2-core-foundation) | `0.3.2` | `0.3.2` | `0.3.2` | 最新 | Hayate |
| [objc2-core-text](https://crates.io/crates/objc2-core-text) | `0.3.2` | `0.3.2` | `0.3.2` | 最新 | Hayate |
| [objc2-foundation](https://crates.io/crates/objc2-foundation) | `0.3.2` | `0.2.2`<br>`0.3.2` | `0.3.2` | 複数版（更新あり） | Hayate |
| [oxipng](https://crates.io/crates/oxipng) | `9.1.5` | `9.1.5` | `10.1.1` | 更新あり | Hayate |
| [parlance](https://crates.io/crates/parlance) | `0.1.0` | `0.1.0` | `0.1.0` | 最新 | Hayate |
| [parley](https://crates.io/crates/parley) (vendored) | `0.9`<br>`vendored 0.9.0` | `0.9.0` | `0.11.0` | 更新あり | Hayate |
| [parley_data](https://crates.io/crates/parley_data) (vendored) | `0.9.0`<br>`vendored 0.9.0` | `0.9.0` | `0.11.0` | 更新あり | Hayate |
| [peniko](https://crates.io/crates/peniko) | `0.6.0`<br>`0.6.1` | `0.6.1` | `0.6.1` | 最新 | Hayate |
| [png](https://crates.io/crates/png) | `0.18`<br>`0.18.1` | `0.18.1` | `0.18.1` | 最新 | Hayate |
| [pollster](https://crates.io/crates/pollster) | `0.4` | `0.4.0` | `1.0.1` | 更新あり | Hayate |
| [pretty_assertions](https://crates.io/crates/pretty_assertions) | `1.3.0` | `1.3.0` | `1.4.1` | 更新あり | Hayate |
| [read-fonts](https://crates.io/crates/read-fonts) | `0.39.0`<br>`0.39.2` | `0.39.2` | `0.42.0` | 更新あり | Hayate |
| [roxmltree](https://crates.io/crates/roxmltree) | `0.21.1` | `0.21.1` | `0.21.1` | 最新 | Hayate |
| [serde](https://crates.io/crates/serde) | `1`<br>`1.0` | `1.0.228` | `1.0.229` | 更新あり | Hayate |
| [serde_json](https://crates.io/crates/serde_json) | `1`<br>`1.0`<br>`1.0.93` | `1.0.150` | `1.0.151` | 更新あり | Hayate |
| [skia-safe](https://crates.io/crates/skia-safe) | `=0.99.0` | `0.99.0` | `0.99.0` | 最新 | Hayate |
| [skrifa](https://crates.io/crates/skrifa) (vendored) | `0.42`<br>`0.42.0`<br>`0.42.1`<br>`vendored 0.42.1` | `0.42.1` | `0.45.0` | 更新あり | Hayate |
| [slotmap](https://crates.io/crates/slotmap) | `1`<br>`1.0.6` | `1.1.1` | `1.1.1` | 最新 | Hayate |
| [smallvec](https://crates.io/crates/smallvec) | `1.15.1` | `1.15.1`<br>`1.15.2` | `1.15.2` | 複数版（更新あり） | Hayate |
| [softbuffer](https://crates.io/crates/softbuffer) | `0.4` | `0.4.8` | `0.4.8` | 最新 | Hayate |
| [static_assertions](https://crates.io/crates/static_assertions) | `1.1.0` | `1.1.0` | `1.1.0` | 最新 | Hayate |
| [taffy](https://crates.io/crates/taffy) (vendored) | `0.12`<br>`vendored 0.12.1` | `0.12.1` | `0.12.2` | 更新あり | Hayate |
| [thiserror](https://crates.io/crates/thiserror) | `2.0.18` | `1.0.69`<br>`2.0.18` | `2.0.19` | 更新あり | Hayate |
| [tiny-skia](https://crates.io/crates/tiny-skia) | `0.11` | `0.11.4` | `0.12.0` | 更新あり | Hayate |
| [vello](https://crates.io/crates/vello) (vendored) | `0.9`<br>`vendored 0.9.0` | `0.9.0` | `0.9.0` | 最新 | Hayate |
| [vello_encoding](https://crates.io/crates/vello_encoding) (vendored) | `0.9`<br>`0.9.0`<br>`vendored 0.9.0` | `0.9.0` | `0.9.0` | 最新 | Hayate |
| [vello_shaders](https://crates.io/crates/vello_shaders) (vendored) | `0.9`<br>`0.9.0`<br>`vendored 0.9.0` | `0.9.0` | `0.9.0` | 最新 | Hayate |
| [wasm-bindgen](https://crates.io/crates/wasm-bindgen) | `0.2` | `0.2.122`<br>`0.2.126` | `0.2.126` | 複数版（更新あり） | Hayabusa, Hayate |
| [wasm-bindgen-futures](https://crates.io/crates/wasm-bindgen-futures) | `0.4` | `0.4.72`<br>`0.4.76` | `0.4.76` | 複数版（更新あり） | Hayabusa, Hayate |
| [wasm-bindgen-test](https://crates.io/crates/wasm-bindgen-test) | `0.3` | `0.3.72` | `0.3.76` | 更新あり | Hayate |
| [web-sys](https://crates.io/crates/web-sys) | `0.3` | `0.3.103`<br>`0.3.99` | `0.3.103` | 複数版（更新あり） | Hayabusa, Hayate |
| [wgpu](https://crates.io/crates/wgpu) | `29`<br>`29.0.3` | `29.0.3` | `30.0.0` | 更新あり | Hayate |
| [wgpu-profiler](https://crates.io/crates/wgpu-profiler) | `0.27.0` | `0.27.0` | `0.27.0` | 最新 | Hayate |
| [windows](https://crates.io/crates/windows) | `0.62.2` | `0.62.2` | `0.62.2` | 最新 | Hayate |
| [windows-core](https://crates.io/crates/windows-core) | `0.62.2` | `0.62.2` | `0.62.2` | 最新 | Hayate |
| [winit](https://crates.io/crates/winit) | `0.30` | `0.30.13` | `0.30.13` | 最新 | Hayate |
| [yeslogic-fontconfig-sys](https://crates.io/crates/yeslogic-fontconfig-sys) | `6.0.0` | `6.0.1` | `6.0.1` | 最新 | Hayate |

## Gradle / Android

| ライブラリ / ツール | 宣言バージョン | 現在（lock / bundled） | 最新安定版 | 判定 | 利用領域 |
|---|---:|---:|---:|---|---|
| [androidx.appcompat:appcompat](https://search.maven.org/) | `1.7.0` | `1.7.0` | `1.7.1` | 更新あり | Hayate |
| [androidx.core:core](https://search.maven.org/) | `1.13.1` | `1.13.1` | `1.19.0` | 更新あり | Hayate |
| [androidx.games:games-activity](https://search.maven.org/) | `3.0.5` | `3.0.5` | `4.4.2` | 更新あり | Hayate |
| [com.android.application](https://plugins.gradle.org/) | `8.13.2` | `8.13.2` | `9.3.0` | 更新あり | Hayate |
| [com.facebook.fbjni:fbjni](https://search.maven.org/) | `0.7.0` | `0.7.0` | `0.7.0` | 最新 | Hayate |
| [com.google.android.gms:play-services-code-scanner](https://search.maven.org/) | `16.1.0` | `16.1.0` | `16.1.0` | 最新 | Hayate |
| [com.squareup.okhttp3:okhttp](https://search.maven.org/) | `4.12.0` | `4.12.0` | `5.4.0` | 更新あり | Hayate |
| [Gradle](https://gradle.org/releases/) | `8.13` | `8.13` | `9.6.1` | 更新あり | Hayate |
| [org.jetbrains.kotlin.android](https://plugins.gradle.org/plugin/org.jetbrains.kotlin.android) | `1.9.24` | `1.9.24` | `2.4.10` | 更新あり | Hayate |
| [org.mozilla.rust-android-gradle.rust-android](https://plugins.gradle.org/) | `0.9.6` | `0.9.6` | `0.9.6` | 最新 | Hayate |

## 対象外

- ワークスペース内パッケージ間の `workspace:*` / `path` 依存（外部最新版がないため）
- 推移的依存だけのパッケージ（直接更新せず、上位依存の更新で解決されるため）
- Android の `compileSdk` / `targetSdk` / `minSdk`、Java target、NDK（ライブラリではなくプラットフォーム設定のため）
- Node.js / Rust toolchain（このリポジトリには `.node-version` / `.nvmrc` / `rust-toolchain.toml` の固定がないため）
