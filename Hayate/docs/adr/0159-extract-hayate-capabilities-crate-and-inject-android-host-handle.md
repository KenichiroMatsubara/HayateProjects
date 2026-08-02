# capability 契約を `hayate-capabilities` crate へ抽出し（鉄則1 は「Core 層が所有」の意味と明文化）、Android leaf は実行環境を ambient global ではなく注入で受ける

status: accepted

Date: 2026-08-02

## Context

Torimi が Tauri 製シェルを持ち、Live Preview（WebView 上の任意 Web アプリ）と Bundle Preview
（Hermes ＋ Hayate）の 2 系統を同一 APK で提供することになった（Torimi ADR-0006 / 0007）。両系統は
同じネイティブ capability を使いたい。ここで守るべき制約は「**Tauri plugin を共通実装の正本にしない**」
こと — Hayate が Tauri の lifecycle・`AppHandle`・WebView・IPC を知る構造にすると、共通化ではなく
Hayate の Tauri 従属になる。

2 つの問題が立つ。

**1. 契約の収容先。** capability の契約は現在 `hayate-core` にある。`crates/platform/README.md` の
鉄則1「契約は Core」（ADR-0117 / 0119 が繰り返し引く）はこの配置を支えている。しかし `hayate-core`
は `taffy` / `parley` / `fontique` / `skrifa` / `accesskit` / `imbl` に依存する重い crate で、
Tauri 側の adapter が haptics を 1 つ呼ぶためにレイアウト・テキスト整形エンジン一式を引き込むことに
なる。一方 capability 群（`capability` / `haptics` / `qr_scanner` / `geolocation` / `sensors` /
`battery` / `biometric` / `device_info` / `local_notification` / `url_launcher` ほか）は調べると
**閉じたクラスタ**で、外部参照は `capability::CapabilityError` と `subscription::Subscription`、
あとは std のみ。`element` / `render` や重い依存への参照は無い。

**2. Android の実行環境の供給元。** 既存の `jni_bridge.rs` は `ndk_context` から JavaVM と Context を
取るが、そこへ値を入れているのは `android-activity` の初期化である。さらに取得できるのは Application
Context であって Activity ではなく、Activity 実体は Kotlin 側のレジストリで解決し、アプリの Kotlin
クラスは Activity の ClassLoader 経由でしか掴めない。Tauri Activity が前面にいる間、`android-activity`
は動いていない。つまり **JavaVM / Context / ClassLoader の供給元が host によって変わる**。これが
この統合で最も硬い縫い目であり、Activity が共存できるかどうかという表面的な問題の実体でもある。

## Decision

- **`hayate-capabilities` crate を切り出す。** capability の契約・型・エラー・共有ロジックを移し、
  `subscription` も同伴させる（stream capability が依存しているため、置き去りにすると循環する）。
  `hayate-core` からは re-export し、既存の呼び出しパス（`hayate_core::haptics::Haptics` 等）を
  変えない。
- **鉄則1 を「契約は Core *層* が所有する」と明文化する。** 守りたいのは「契約を adapter / leaf 側に
  持たせない」ことであって、特定の crate に物理的に置くことではない。本抽出は adapter への移譲では
  ないので鉄則1 に反しない。逆に、Tauri plugin 側に別契約を定義して境界で変換する案は二重契約に
  なるため採らない。
- **Android leaf は実行環境を注入で受ける。** `hayate-capabilities` の Android 実装は `ndk_context`
  等の ambient global を直接読まず、JavaVM・Application Context・現在の Activity をまとめた
  host handle を**引数として**受け取る。各 host adapter がそれを構築して渡す:
  - Hayate 側 adapter は `android-activity` が設定した値から構築する（既存の `with_activity_env` は
    この adapter の内部実装へ降格する）。
  - Tauri plugin は Tauri の Activity から構築する。
  Activity レジストリと ClassLoader 解決の作法は host ごとに実装が異なるため、共有せず adapter 側に置く。
- **共有 Module は host を知らない。** `hayate-capabilities` は Tauri にも Hayate ランタイムにも
  依存しない。Torimi Command と Hayate 標準ライブラリは、同じ Module を別 Adapter から使う。
- **platform leaf の Kotlin / Swift は残す。** OS 呼び出しは完全には Rust 化できない（例: QR は
  Play services の Kotlin/Java API しか経路が無い・ADR-0125）。leaf は薄く保つ。

## Considered Options

- **`hayate-core` に据え置き、Tauri adapter に core ごと依存させる。** 抽出コストはゼロだが、
  adapter が `parley` / `taffy` / `accesskit` を引きずり、Tauri 側にだけ必要な変更が core に入る
  圧力が残る。
- **ambient global を維持し、共有実装を両方の cdylib に static link する。** `ndk_context` の static は
  shared object ごとに別インスタンスなので、各 `.so` が自分の host が設定した値を見て**偶然
  整合する**可能性がある。しかし動く理由が `.so` 境界という暗黙の事情に依存し、後から触る者が踏む
  地雷になる。採らない。
- **両者を単一 cdylib に統合する。** 上記グローバルが衝突する。採らない。

## Consequences

- 統合の検証は QR スキャナで行う。Android leaf で実装済みなのは `audio_output` と `qr_scanner` の
  2 つだけで、`haptics` を含む他はすべて `Unimplemented` を返す stub である。かつ `qr_scanner` は
  Activity Context・ClassLoader 解決・別 Activity の結果受け取りという**今回の難所をすべて通る唯一の
  経路**であり、Torimi Shell 自身も QR を必要とする。haptics は Application Context だけで足りるため
  縫い目を迂回してしまい、通っても統合の証拠にならない。
- 検証では使用フレームワークが提供する既製のバーコードスキャナ機構を用いない。用いると「共有 Module
  経由でネイティブに届く」ことを何も証明しないため。
