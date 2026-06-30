# QR スキャナを Mobile Family Adapter の capability として iOS/Android 単一 API で扱い、Android は Google Code Scanner（本アプリ初の Rust↔Kotlin JNI seam）・iOS は VisionKit DataScannerViewController で両ネイティブ実機実装する

status: accepted

Date: 2026-06-29

## Context

Miharashi（framework 非依存の dev-client）は、事前ビルド済みネイティブホストが dev-server から
App Bundle を実行時 fetch する（Miharashi CONTEXT.md / ADR-0001）。接続先 dev-server は端末上で
指定する（#534・`dev_server_target`）。これを楽にするため、起動コマンド（`@miharashi/dev-server`）が
**ローカルネットワーク URL** を端末に QR で表示し、スマホのカメラでそれを読み取って URL を入れる、
という流れを入れた。

ここで「スマホのカメラで読む」を **iOS ネイティブと Android ネイティブの両方**で実装する必要がある。
重要な前提整理：

- **今作っているのは Tsubame であり、その iOS ホストはネイティブ（ADR-0114：UIKit/Metal + 薄い
  Swift host、`hayate-adapter-ios`）である。** Miharashi の iOS ホストも同じネイティブ路線で立つ。
- ADR-0121（webview+wasm）は **Hayabusa を将来 iOS に載せる場合**の経路であって、Tsubame/Miharashi の
  iOS ホストが web 経由になるという意味ではない。**「iOS は web ルート」と読めるなら、それは誤読
  （ないし記述の不備）であり、本 ADR で否定する。** カメラ QR も iOS ネイティブで実装する。

最初の実装（先行コミット）は Android の `DevServerSetupActivity`（Kotlin）に Google Code Scanner を
直書きしただけで、iOS が無く、Rust の capability 契約も通っていなかった。これは ADR-0117 が定めた
「iOS/Android を**単一 API**で扱う」機構（Mobile Family Adapter）を素通りしており、設計として誤り
だった。Web 側（`@miharashi/host-web` の `scanQrFromCamera`、標準 `BarcodeDetector`）は ADR-0117 が
言う **family-of-1**（web は Family Adapter を持たず leaf を直接置く）なので、それ自体は正しい。

## Decision

QR スキャンを **Core capability** として定義し、ADR-0117 / ADR-0119 の流儀に揃える。

- **契約は Core**：`hayate_core::qr_scanner`（`trait QrScanner { fn scan() -> Result<Option<ScannedCode>, CapabilityError> }`、
  値型 `ScannedCode { value: String }`）。`file_picker` と同型の **async-UI 一発取得**で、キャンセルは
  `Ok(None)`。呼ぶと native UI が出て結果まで**ブロック**する（呼び側は UI スレッド外から呼ぶ）。
- **単一 API は facade**：`hayate-adapter-mobile` の `MobileQrScanner` が `cfg(target_os)` で leaf を
  解決する。上位は leaf を名指しせず `MobileQrScanner` だけを参照し、**iOS/Android を 1 つの型名**で
  扱う（ランタイム dispatch ではなく cargo のターゲット別リンク・ADR-0117）。
- **Android leaf は実機実装**：`hayate_adapter_android::qr_scanner::AndroidQrScanner`。バックエンドは
  **Google Code Scanner**（`play-services-code-scanner`）。Play services がスキャナ UI とカメラ取得を
  内包するので CameraX も独自カメラ権限も要らない。
- **iOS leaf も実機実装**：`hayate_adapter_ios::qr_scanner::IosQrScanner`。バックエンドは **VisionKit
  `DataScannerViewController`**（iOS 16+）。`audio_output`（`hayate_ios_audio_*`）と同型に native は
  `hayate_ios_qr_*` の薄い C FFI に閉じ、Swift ホスト（ios-app の `QrScanner.swift`）が `@_cdecl` で
  実装する（ADR-0114 shape 1：Swift が UIKit/VisionKit を持ち Rust は ObjC-free）。stub は卒業し、
  `audio_output` と同じく専用モジュールへ昇格した（capability_stubs には残さない）。
- **Web は family-of-1 のまま**：`@miharashi/host-web` の `scanQrFromCamera`（`BarcodeDetector`、
  非対応ブラウザは手入力フォールバック）を直接 leaf として持つ。Family Adapter には載せない。
- **bootstrap UI は leaf を共有する**：Android の `DevServerSetupActivity`（端末で dev-server URL を
  入れる pre-host 画面）の「QR スキャン」は、独自実装を持たず **`QrScannerBridge`**（Android leaf の
  実体）を呼ぶ。`QrScannerBridge` は Rust capability 用の同期入口 `scanBlocking`（JNI から worker
  スレッドで呼ぶ）と、bootstrap UI 用の非ブロック入口 `startScan` を持つ。**Android の QR 実装は 1 つ**。

### Rust↔Kotlin JNI seam（本アプリ初）

`audio_output` は「Kotlin を介さず純 NDK」で書けたが、Google Code Scanner は **Play services の
Kotlin/Java API しか無く NDK 経路が無い**。QR デコードを純 native で持つには独自デコーダ実装が要り
非現実的なので、本 leaf は **本アプリ初の Rust↔Kotlin JNI seam** になる：

- 依存は android ターゲットのみに `jni` / `ndk-context` を追加。
- `AndroidQrScanner::scan()` は `ndk_context` から JavaVM と Activity(Context) を取り、現在スレッドを
  attach して `QrScannerBridge.scanBlocking(activity)` を静的呼び出しする。null = キャンセル。
- 汚い FFI glue は `#[cfg(target_os="android")]` に封じ込め、host では契約再露出だけをコンパイルする
  （`audio_output` と同パターン）。

## Consequences

- 上位（Miharashi ホスト / 将来の Tsubame アプリ）は `MobileQrScanner` 一本で iOS/Android の QR
  スキャンを呼べる。両 leaf が実機実装済みで、capability は対称（昇格は 2 実装から・ADR-0117 を満たす）。
- **実機検証はサンドボックス外**：Android の JNI/Code Scanner（Gradle + NDK + Play services）も iOS の
  VisionKit FFI（Mac/iOS SDK + カメラ実機）も host では検証できない（`audio_output` の AAudio/
  AVAudioEngine FFI と同じ扱い）。host では Core 契約・facade 解決・各 leaf の純粋部をコンパイル/
  テストし、汚い FFI glue は encapsulation guard（`tests/qr_scanner_encapsulation.rs`）で各 leaf 内に
  封じ込めることをソース走査で固定する。
- iOS は VisionKit、Android は Code Scanner という別 native UI だが、`MobileQrScanner` という単一 API の
  裏に隠れる。カメラ権限は iOS が `NSCameraUsageDescription`（Info.plist）、Android は Play services の
  スキャナ UI が内包（宣言不要）と差があるが、これも facade の外側（packaging）の差として閉じる。
- `DevServerSetupActivity` は独自スキャナを捨てて leaf を共有するので、Android の QR 実装の二重化が
  無くなる。pre-host bootstrap が Rust capability ではなく Kotlin 入口を直接使うのは、bootstrap が
  Rust ホスト起動**前**に動く Android 固有 UI だからで、跨プラットフォーム API（`MobileQrScanner`）の
  単一性とは独立（同じ leaf 実体を共有することで実装の単一性は保つ）。
