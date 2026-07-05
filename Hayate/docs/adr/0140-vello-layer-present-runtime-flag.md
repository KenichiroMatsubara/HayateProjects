# vello の layer-present をランタイムフラグ化し pkg-layer-present を廃止する

**Status: accepted**

**Date: 2026-07-05**

## Context

`Hayate/crates/platform/web/src/backend/vello.rs` の per-layer 経路（#690・ADR-0125/0127）は `#[cfg(feature = "layer-present")]`（コンパイル時 cargo feature、既定 OFF）でゲートされており、これに対応するため `hayate-adapter-web`（OFF）と `hayate-adapter-web-layer-present`（ON）という2つの wasm-pkg を別々にビルドし、`Hayate/host/src/load-canvas-backend.generated.ts` の `loadCanvasBackend` が `layerPresent` 引数の値でどちらを動的 import するか選んでいた（ADR-0135 の本人調査用トグル起源、`9ba1718`）。

tiny-skia・vello_cpu の per-layer 経路は ADR-0138（#710）で既に「cargo feature も別 wasm-pkg も使わず、常時コンパイルされたコードをランタイムの bool フィールドで on/off する」形に揃っており、ADR-0138 の Consequences は「vello の `layer-present` cargo feature をランタイムフラグ化し `pkg-layer-present` を廃止できないか、という論点は本 ADR のスコープ外——別 issue で追う」と明記していた。issue #711（`/grill-with-docs` セッション）でこの宿題を検討した結果、次の2点が判明した。

- `layer-present` cargo feature は `hayate-adapter-web`（Web 専用クレート）にしかスコープされておらず、native（Android/iOS）はこの feature に一切依存していない——native 側の旧 per-layer 実装自体が #687 で撤去済みのため。ADR-0135 が定める「native では製品として有効化しない」という封印は、この cargo feature の有無とは無関係に、native に per-layer 実装が存在しないことで保たれている。
- ADR-0137 により Web の既定は既に ON（`loadCanvasBackend` の `layerPresent` 引数は既定 `true`）になっているため、通常の Web 訪問者は既に常時 `hayate-adapter-web-layer-present`（GPU パイプライン warmup 込みの大きい方のバイナリ）をダウンロード・実行していた。素の `hayate-adapter-web` は `?layerPresent=0` という明示的な逃げ道からしか到達しない副経路になっていた。

つまり2バイナリ構成は、もはやユーザー向け挙動の安全装置としては機能しておらず、`pkg-layer-present` という追加ビルド成果物・`build:layer-present` スクリプト・`wasm-build-manifest.json` の専用エントリ・deploy-pages.yml の追加コメント・npm エイリアスという、保守コストだけを積み上げる重複ビルド経路として残っていた。

## Decision

`hayate-adapter-web` の `layer-present` cargo feature を廃止し、`vello.rs` の per-layer コードを常時コンパイルした上で、`SelectedBackend` に `layer_present_enabled: bool` フィールド（既定 `true`、tiny-skia/vello_cpu と同じ形）を持たせてランタイムで on/off する（実装: issue #717・#718）。

tiny-skia/vello_cpu と異なり、vello の per-layer 経路は `VelloLayerRasterizer`（GPU device/queue を握る）・`WgpuQuadCompositor`（`warmup()` で GPU パイプラインを前倒しコンパイルする、ADR-0130a）という実 GPU リソースを伴う。これらは `set_layer_present_enabled(true)` が呼ばれて初めて construct・warmup する**遅延初期化**にする——tiny-skia/vello_cpu の「常時 construct」パターンをそのまま踏襲すると、`?layerPresent=0` を選んだユーザーが使わない GPU パイプラインの初期化コストを新たに払うことになるため。construct が失敗した場合は、既存の vello warmup 失敗ハンドリング（boot を落とさず警告ログのみで続行する）に倣い、`layer_present_enabled` を `false` のまま全面 raster にフォールバックする。

`pkg-layer-present` の wasm-pkg・`build:layer-present` スクリプト・`wasm-build-manifest.json` の専用エントリ・`hayate-adapter-web-layer-present` の npm エイリアス・deploy-pages.yml の関連コメントを削除する。`loadCanvasBackend` の公開 API（`layerPresent`・`cpuLayerPresent` という別々の引数名）は変えない——vello と tiny-skia/vello_cpu の per-layer トグルは今も独立した意味を持つ別物であり、ここを一本化するのは本 ADR のスコープ外。

## Considered Options

- **現状（2 wasm-pkg・cargo feature）を維持する**: ADR-0135 制定当時は「製品として有効化しない」ことの物理的な保証として意味があったが、ADR-0137 で Web 既定が ON になった時点でその役割は既に終わっており、以後は保守コストだけが残る。却下。
- **vello も tiny-skia/vello_cpu と全く同じ「常時 construct」パターンにする**: 実装が単純になるが、vello の compositor/rasterizer は実 GPU リソースを伴うため、`?layerPresent=0` の調査用逃げ道を選んだユーザーが不要な GPU 初期化コストを払うことになる。却下し、遅延初期化を採用。
- **`layerPresent`（vello）と `cpuLayerPresent`（tiny-skia/vello_cpu）を1つの引数に統合する**: ランタイムフラグという点では同じ機構になったが、vello は ADR-0135 の非推奨注記付き、tiny-skia/vello_cpu は ADR-0138 で既知バグなしの単純比較用と、意味は今も別物であり、Tsubame UI も別ボタン・別クエリキーとして扱っている。本 issue のスコープ外として却下。

## Consequences

- `hayate-adapter-web/Cargo.toml` から `layer-present` feature 定義とその説明コメントが消える。「製品としては有効化しない」という ADR-0135 の封印意図は、以後 `vello.rs` の `layer_present_enabled` フィールドのコメントと本 ADR に記録される——cargo feature という物理的な仕組みではなく、既定値とコメント・ADR による運用上の取り決めになる。
- `pkg-layer-present`・`build:layer-present`・`hayate-adapter-web-layer-present`・関連 CI コメントが削除される（実装: issue #718）。
- ADR-0138 の Consequences に記載されていた「vello のランタイムフラグ化は別 issue で追う」という宿題は本 ADR と issue #711/#716/#717/#718 で解消済みとする。
- 本 ADR は ADR-0135 が定める native への封印方針そのものは変更しない——本 ADR が扱うのは Web 専用クレートのビルド構成のみであり、native は元々この cargo feature の対象外だった。

## 関係

- **amends** ADR-0135（layer-present 封印。「製品としては有効化しない」の実現手段が cargo feature からランタイムフラグの既定値へ変わることを追記し、native はこの cargo feature に元々依存していなかったという事実を明記する）。
- **references** ADR-0137（Web の layer-present 既定 ON——本 ADR の前提となる、2バイナリ構成が既に安全装置として機能していなかったことの根拠）、ADR-0138（tiny-skia/vello_cpu が先行して確立したランタイムフラグパターン、および本 ADR が解消する宿題の出典）。
- 動機となった議論: issue #711（`/grill-with-docs` セッション）、PRD #716、実装 issue #717・#718。
