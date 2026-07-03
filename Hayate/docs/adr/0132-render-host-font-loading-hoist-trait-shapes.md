# ADR-0068 の hoist を実装に落とす — `Surface`/`FontFetcher` は core 所有・GPU経路専用・同期片方向に具体化する

**Status: accepted**

**Date: 2026-07-03**

## Context

ADR-0068（プラットフォーム非依存の Render Host / Font ロードを共有層へ hoist する、accepted）は決定から実装が一切進んでいなかった。`hayate-app-host` crate（ADR-0117 の App Host boot seam の実装）が hoist 先として既に存在し、tick ループ・`DeliverySink`・最小 `Surface` trait（`fn present(&mut self, scene: &SceneGraph)` のみ）まで実装済みだが、`hayate-adapter-web` は `hayate-app-host` に依存すらしておらず、`RendererSelectionPolicy` / `RenderHost` / font queue はすべて `crates/platform/web` 内に留まったままだった（`/improve-codebase-architecture` の監査で確認）。

ADR-0068 は共有層の trait 形を「`Surface`：acquire / present / resize / configure」「`FontFetcher`：`fetch(url) -> bytes`」と書いたが、実際に hoist を設計する過程で、この2点は実装レベルでは成立しないことが分かった。

- `Surface` を GPU（Vello/wgpu）経路と CPU（tiny-skia）経路の両方に単一 trait で対応させようとすると、両者が要求する資源型が根本的に異なる（wgpu Surface vs canvas 2D コンテキスト + Pixmap）ため、一つの trait に押し込むと shallow になる。
- `FontFetcher::fetch(url) -> bytes` を素直に async trait として core/app-host に持たせようとすると、「platform非依存であるはずの共有層がどうやって非同期実行（web の `spawn_local` 相当）を駆動するか」という、この codebase にまだ存在しない抽象化問題を新規に作ってしまう。

既存の `ImeBridge` trait（`crates/core/src/element/ime_bridge.rs`）が、ADR-0117（三層モデル、`docs/adr/0117-adapter-core-seam-three-layer-model.md`）で「`Surface`/`FontFetcher` と同型」と名指しされている先例であり、core 所有・同期・単方向（`fn present(&mut self, presentation: ImePresentation)`、`ElementTree::drive_ime` が駆動）という形を取っている。この形に倣うことで両方の問題が解決した。

## Decision

ADR-0068 の Decision（Render Host と Font ロードを共有層へ hoist する）自体は維持する。trait の形と置き場所を以下のとおり具体化・訂正する。

### スライス1: Renderer Selection Policy

- `RendererSelectionPolicy` / `RendererSelectionPlan` / `RendererSelectionReason` / `SceneRendererKind`（非wasm部分）/ `is_runtime_fallback_reason`（現 `crates/platform/web/src/renderer_selection.rs`）を `hayate-app-host` へ hoist する。既に `wasm-bindgen`/`web-sys` 非依存の純粋関数・型であり、既存ユニットテストもそのまま移設できる。
- `classify_init_error`（現 `backend/mod.rs:234`、`&JsValue` を受け取り文字列マッチで判定）は **hoist しない**。中身を調べると `"webgpu"` `"adapter not found"` `"surface lost"` は wgpu 自身のエラー語彙（ADR-0002: wgpu が唯一の Backend）で platform 非依存だが、`"context unavailable"` `"failed to cast"` は `backend/tiny_skia_backend.rs` の DOM canvas 2D コンテキスト取得・JS キャスト失敗という web 固有のエラー形状で、両者が1関数に混在している。丸ごと hoist すると web 固有知識を共有層に持ち込み、是正したい違反を一段深いところで再生産する。**共有するのは `RendererSelectionReason` という語彙（enum）のみ**とし、分類ロジック自体は各 platform adapter（web、将来の desktop）が個別に実装する。

### スライス2: Font Loading

- `FontFetcher` trait を **`hayate-core`** に定義する（`ImeBridge` と同じ置き場所）。形は ADR-0068 原文の `fetch(url) -> bytes` ではなく、`ImeBridge::present` に倣った **`fn request(&mut self, family: &str)` という同期・発火のみの片方向メソッド**にする。
- core に `ElementTree::drive_font_requests(&mut self, fetcher: &mut impl FontFetcher)` を新設する（`drive_ime` と同型）。欠落フォントを検出するたびに `fetcher.request(&family)` を同期に呼ぶ。現在 web adapter が `tree.poll_events()` から `Event::FetchFont` を手動 poll しているループ（`canvas.rs:1086-1124`）の core 側での置き換えになる。
- URL解決（`font_url_for_renderer`、GPU経路でのカラー絵文字格上げ判断込み、ADR-0043領域）は hoist しない。監査で「正しく adapter 所有」と確認済みで、`impl FontFetcher::request` の中で adapter が行う。
- 実際の非同期フェッチ（`spawn_local` + fetch、指数バックオフ）は100% adapter 実装内に閉じる。**core も app-host も executor を一切知らない — 非同期実行の抽象化そのものを導入しない。**
- 完了報告（成功バイト列／失敗ファミリ名）は `hayate-app-host` が所有する mailbox（キュー）を介す。これは capability contract（`FontFetcher`）とは別の理由で必要になる機構 — 単一スレッド WASM で `spawn_local` された非同期クロージャが `&mut ElementTree` を安全に横断 borrow できない（tick ループが排他的に tree を持つ間しか安全に書けない）という再入問題を避けるため。`AppHost` は構築時にこのキューへの clone ハンドルを公開し、adapter の `impl FontFetcher` はそれを保持して非同期クロージャ内から結果を push する。`AppHost::tick()` は毎フレーム、layout より前にこのキューを drain して `tree.register_font` / `tree.font_fetch_failed` へ流し込み、既存の「フォント登録は layout より前」という順序不変条件（`canvas.rs:427-444`）を保つ。
- リトライ予算の断念判断は既に core（`tree.font_fetch_failed` の戻り値）が持ち、変更なし。バックオフのタイミング定数はネットワークチューニングとして adapter 側に残す。

### スライス3: Render Host / Surface

- `Surface` trait を **`hayate-core`** に定義する（`ImeBridge`/`FontFetcher` と統一の置き場所）。現在 `crates/app-host/src/lib.rs` が自分自身の中で定義しているのは、ADR-0117 三層モデルの「capability の契約は常に Core 所有」原則と矛盾しており、本 ADR で是正する。
- `Surface` trait のスコープは **GPU（wgpu）経路専用**に限定する。Vello 経路は `wgpu::Instance::create_surface(wgpu::SurfaceTarget::...)` という wgpu 自身が web/desktop 横断で既に提供する抽象に乗るだけで済むが、tiny-skia（CPU）経路は wgpu を一切使わず canvas 2D コンテキストへの直接 blit という全く別の資源型を必要とする。ADR-0048 が記す tiny-skia 導入理由は「WebGPU のブラウザ間対応状況のばらつき（Firefox/Safari<17.4/GPUなしCI）」という web 固有の事情であり、ADR-0118 は「tiny-skia（CPU）は確認用の位置づけで、desktop の本番 Surface には据えない」と明記する。CPU-present 経路の本番消費者は web 一つしかなく、"一 adapter = 仮説の seam" 原則によりここでの共有抽象化は見送る。tiny-skia の CPU present 経路は `hayate-adapter-web` に残置する。
- `RenderHost` struct（現 `backend/mod.rs:271`、`canvas: HtmlCanvasElement` を所有）と `SceneRenderer` trait（現 `Result<(), JsValue>` を返す）を `hayate-app-host` へ hoist する。`canvas: HtmlCanvasElement` は `impl Surface`（core定義）に置き換える。エラー型は `JsValue` ではなく **`anyhow::Error`** にする（`hayate-core` は既に `anyhow` に依存済み。`classify_init_error`（adapter側に残置、スライス1参照）は `error.to_string()` を見ればよく、Hayate 独自のエラー enum は「結局文字列マッチに戻る」ため見送った）。
- レンダラー初期化（`try_init` 等）は起動時に一度だけ呼ばれる非同期処理で、tick ループがまだ回っていないため font のような再入問題は発生しない。したがって mailbox パターンは不要で、`async fn` のまま素直に hoist してよい。

## Considered Options

- **`Surface`/`FontFetcher` を ADR-0068 原文どおりの形（acquire/present/resize/configure、async fetch(url)->bytes）で hoist する**：ADR-0068 に最も忠実だが、`Surface` は GPU/CPU 両経路を無理に一つの trait に収めることになり shallow になる。`FontFetcher` は app-host/core に非同期実行の抽象化を持ち込む新規問題を作る。却下。
- **`Surface` を GPU/CPU 両対応の enum ベース抽象（`PresentTarget::Gpu(...)`/`PresentTarget::RawPixels(...)`）にする**：一つの trait 項目には収まるが、呼び出し側（`RenderHost`）が結局 variant ごとに全く別の処理をすることになり、二つの trait に分けるのと実質同じ複雑さを1ファイルに押し込むだけ。しかも CPU 側の実消費者は web 一つしかなく、今 CPU 側の形を確定させる情報がない（"一 adapter = 仮説の seam"）。却下。
- **CPU（tiny-skia）present 経路も今回一緒に共有化する**：desktop が将来 embedded/GPU非搭載環境向けに CPU フォールバックを必要とする可能性をユーザーが指摘したが、ADR-0048・ADR-0118 および "embedded"/"組み込み" の文書検索では裏付けが見つからず、ADR-0118 は明示的に「tiny-skia は desktop の本番 Surface には据えない」としている。確証のないまま抽象を先取りするのは尚早と判断し見送った。

## Consequences

- `hayate-adapter-web` が `hayate-app-host` に依存するようになる（現状は依存なし）。
- `hayate-core` に `FontFetcher` trait・`Surface` trait（GPU専用）・`ElementTree::drive_font_requests` が追加される。`hayate-app-host` に `RendererSelectionPolicy` 一式・font mailbox・`RenderHost`（`anyhow::Error` ベース）が追加される。
- `classify_init_error` は hoist されず、将来 desktop adapter も自分自身のエラー文言に対する分類を個別に実装する必要がある（`RendererSelectionReason` という共有語彙を返す点だけが揃っていればよい）。
- CPU（tiny-skia）present 経路は今回 hoist しない。**将来 desktop や他 platform が embedded/GPU非搭載環境向けに本番の CPU フォールバックを具体的に必要とした時点で、その実データをもとに CPU-present 抽象を再検討する。**（ユーザーが「組み込み」用途の記憶を示唆したが本セッションでは裏付けを確認できなかった — 今後裏付けが見つかった場合はこの ADR の CPU-present 見送り判断を revisit する。）
- `crates/app-host/src/lib.rs` 現行の `Surface` trait（`present()` のみ）は本 ADR の hoist 作業で core へ移設・拡張され、app-host 自身の定義は削除される。
- ADR-0118（`docs/adr/0118-desktop-platform-front-winit-vello-single-crate.md:9`）の「`Surface` trait...は揃っており」という記述は、本 ADR 時点の実態（app-host の `present()` のみの最小スタブ）と食い違っており、本 ADR の hoist 完了後に改めて事実確認・訂正が必要。

## 関係

- ADR-0068：本 ADR が hoist の実行計画として具体化する対象。Decision（hoist する）自体は維持し、trait 形（`Surface`/`FontFetcher`）のみ訂正する。
- ADR-0117（`docs/adr/0117-adapter-core-seam-three-layer-model.md`）：「capability の契約は常に Core 所有（`ImeBridge`/`Surface`/`FontFetcher` と同型）」の原則を本 ADR がそのまま踏襲する。同一の ADR 番号 0117 が `docs/adr/0117-app-host-boot-seam-tick-loop-request-redraw-push-delivery-sink.md`（App Host の tick/`DeliverySink` の実装 ADR）にも振られており、本 ADR はそちらの hoist 先実装（`hayate-app-host`）にも直接依存する。番号重複はこの ADR の関知するところではなく別途棚卸しが必要。
- ADR-0069（`docs/adr/0069-text-input-editstate-and-imebridge.md`）：`ImeBridge` trait の定義元。本 ADR の `FontFetcher`/`Surface` の形（core 所有・同期・単方向）はここでの `ImeBridge` の形を先例として踏襲する。
- ADR-0002（wgpu as sole GPU backend）：`classify_init_error` を hoist しない判断・`Surface` を GPU 専用に限定する判断の両方が、wgpu がクロスプラットフォームな唯一の GPU Backend であるという本 ADR の前提に依拠する。
- ADR-0048（tiny-skia CPU fallback）・ADR-0118（desktop platform front）：tiny-skia の CPU present 経路を今回 hoist しない根拠。両 ADR とも tiny-skia を web 固有ないし確認用の位置づけとしている。
