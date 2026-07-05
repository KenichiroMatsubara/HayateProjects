# layer-present（per-layer 経路）を封印する — 実ブラウザで描画バグを確認したため

**Status: sealed — 有効化禁止（Decision 参照）**

**Date: 2026-07-05**

## Context

ADR-0125/0127 で設計し #690 で実装した per-layer 経路（`layer-present` cargo feature、既定
OFF）は、native `cargo test`（`WGPU_BACKEND=vulkan`）の golden-pixel parity・perf probe
（#691・#692）までは通っていた。#697 で実 Chromium（通常ブランチ、
`--enable-unsafe-webgpu --ignore-gpu-blocklist --use-angle=vulkan`）に layer-present ON の
WASM ビルドを実際に読み込ませ、AddForm の優先度セグメントトグルを目視確認したところ、
性能の優劣を計測する以前に **描画そのものにバグが多数あり、実用に耐える段階ではない**こと
が判明した。native の golden-frame テストはこの不具合を捕捉していない — 実ブラウザ固有
（あるいは native harness が再現しない条件）の問題ということになる。

## Decision

`layer-present` 経路のコード（`hayate-app-host::render_host::{supports_layer_present,
present_layers}`、`platform/web/src/backend/vello.rs` の `#[cfg(feature = "layer-present")]`
実装、`hayate-scene-renderer-vello::layer_compositor` 等）は **削除しない**が、**いかなる
ビルド・製品でも有効化しない**。既定 OFF を維持し、feature ごと凍結する。

Core は本経路を **現状非推奨（do-not-use、ADR-0135）** として扱う。再開条件は「実機/実
ブラウザで計測可能な性能上の実害が具体的に発生した時」のみとし、それまでは全面描画
（`render_scene`、layer-present OFF 相当）のみを正式経路とする。golden-pixel parity テスト
（#691）・perf probe（#692）・#697 の実ブラウザ e2e harness はコードベースに残し、再開時
の回帰ガード／出発点として保存する。

再開する場合は、実ブラウザでの描画バグ修正を再開の必須ゲートとする（native テストのみ
での再開は不可 — それだけでは今回と同じ見落としを繰り返す）。

## Consequences

- `layer-present` feature は cargo 上ビルド可能なままだが、コード中に「ADR-0135 まで
  有効化禁止」の注釈を付す。
- ADR-0125・ADR-0127 のロールアウト（Phase 2 以降のバックエンド半分）は本 ADR により凍結
  される。
- Tsubame 側の #697 e2e harness（`playwright.config.layer-present.ts` 等）は削除せず維持
  するが、既定の `pnpm test:e2e` には含めない現状を継続する。

## 関係

- **freezes** ADR-0125（compositing layer incremental rendering、Phase 2 backend half）,
  ADR-0127（layer cache memory budget / scroll overscan）。
- 動機となった実測: #697（Playwright 実 Chromium 検証、実ブラウザでの描画バグ発見）。
