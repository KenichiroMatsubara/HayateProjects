# Vendored third-party native dependencies (ADR-0112)

Tsubame の JS を Android 実機で動かすため、埋め込み Hermes（JSI）を cdylib に
リンクする。Hermes を React Native の外で使うには JSI / fbjni / libc++ が必要だが、
`libjsi.so` / `libfbjni.so` は React Native の `react-android` AAR にしか配布されて
いない。`react-android` に依存すると不要な `libreactnative.so` まで APK に入るため、
**必要な最小限のヘッダと共有ライブラリだけを vendor** する（リポジトリの
vendored-dependencies 方針 = ADR-0007）。

## 取得元 / バージョン

React Native 0.82.1（Maven Central）:

- `com.facebook.react:hermes-android:0.82.1-release`
  - ヘッダ: `prefab/modules/hermesvm/include/hermes/{hermes.h,Public/**}` →
    `third_party/include/hermes/`
  - ライブラリ: `jni/arm64-v8a/libhermesvm.so` →
    `android-app/app/src/main/jniLibs/arm64-v8a/`
- `com.facebook.react:react-android:0.82.1-release`
  - ヘッダ: `prefab/modules/jsi/include/jsi/**` → `third_party/include/jsi/`
  - ライブラリ: `jni/arm64-v8a/libjsi.so` →
    `android-app/app/src/main/jniLibs/arm64-v8a/`

`libreactnative.so` 等、上記以外は取り込まない。

`libfbjni.so` / `libc++_shared.so`、および libfbjni の JNI_OnLoad が要求する Java
クラス `com.facebook.jni.*` は vendor せず、Gradle 依存 `com.facebook.fbjni:fbjni:0.7.0`
（react-android 0.82.1 が使う版）が供給する。fbjni は React 本体ではない汎用 JNI
ヘルパ。

## ライセンス

Hermes および React Native（JSI を含む）は MIT License（Copyright (c) Meta
Platforms, Inc. and affiliates）。各プロジェクトの LICENSE を参照:

- Hermes: https://github.com/facebook/hermes/blob/main/LICENSE
- React Native: https://github.com/facebook/react-native/blob/main/LICENSE

## 更新手順

バージョンを上げる場合は同じ classifier（`-release`）の AAR を取得し、上記の
ヘッダ／.so を同じ配置で置き換える。`build.rs` が `CARGO_MANIFEST_DIR` 相対で
`third_party/include` と `android-app/app/src/main/jniLibs/arm64-v8a` を自動解決する
（`HERMES_INCLUDE` / `HERMES_LIB` env で一時上書きも可能）。
