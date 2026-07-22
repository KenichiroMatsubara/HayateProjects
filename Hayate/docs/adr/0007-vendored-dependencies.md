# 主要依存を workspace 内にベンダリングし、upstream から自律する

Vello・Taffy・parley・fontique・skrifa を `crates/vendor/` として workspace に取り込み、upstream との依存関係を切断する。upstream の変更に引きずられず、Hayate の都合で任意のタイミングに upstream の改良を選択的に取り込む。

## 対象

| crate | 理由 |
|---|---|
| vello | 2D レンダラーの核心。API 変更が Hayate の描画パイプラインに直撃する |
| taffy | レイアウト計算の核心。独自最適化の余地がある |
| parley / fontique / skrifa | テキストスタックの核心。Linebender upstream と足並みを合わせる必要がない |

wgpu は対象外。GPU API 抽象として巨大すぎ、プラットフォーム対応の追従コストが高い。wgpu は Cargo.toml 依存として維持し、メジャーバージョンで評価して移行する。

skia-safe も対象外（wgpu 同類の例外。2026-07 追記、ADR-0146 / issue #799）。Skia は巨大な C++ ツリーで、ソースベンダリングの保守コスト（ビルドシステム・プラットフォーム追従）が利益を上回る。そもそも導入動機が「Android HWUI / Chrome で Google が実証した実績をそのまま使う」ことにあり、Skia に手を入れる意図が無い — 手を入れないなら本 ADR の眼目（所有権・選択的取り込みの判断権）を得る必要も無いため、**fork もしない**。運用は次のとおり:

- Cargo.toml 依存として維持し、crates.io の skia-safe＋ビルド済みバイナリを使う。版は**厳密ピン**（`^` を使わない）。
- CI はビルド済みバイナリをキャッシュし、バイナリ取得込みのクリーンビルドを CI で検証する（ビルド時ネットワーク依存が壊れたらすぐ検知する）。
- hardening 選択肢: GitHub Releases からのバイナリ取得が問題（消失・帯域・到達性）を起こした場合、`SKIA_BINARIES_URL` 系の環境変数でセルフホストミラーへ切り替えられる。記録のみで、既定では使わない。

## Considered Options

- **Cargo.toml 依存として使い続ける**: upstream の破壊的変更に強制追従させられる。substrate の安定性が外部に依存する。
- **ベンダリング（採用）**: Hayate が crate の所有者になる。upstream の bugfix は git の cherry-pick 等で任意に取り込む。

## Consequences

upstream から cherry-pick する運用が必要。ただし「取り込むかどうか」の判断権は常に Hayate 側にある。

## 更新記録（選択的取り込み）

本 ADR の「upstream 改良を選択的に取り込む」運用の実記録。**採用した組み合わせと理由**を残す。

### 2026-07（issue #759）— taffy 0.7.7 → 0.12.1（テキストスタックは据え置き）

**採用した版セット:**

| crate | 版（更新前 → 更新後） | 判断 |
|---|---|---|
| taffy | 0.7.7 → **0.12.1** | **更新**。CSS Gallery「Motion」の wrap 行 × min/max クランプ重なりが 0.12.1 で upstream 修正済み（0.9.2 未修正 → 0.12.1 で PASS を再現確認）。taffy は hayate-core だけが消費し、他 vendor crate と依存を共有しないので独立に上げられる。 |
| vello / vello_encoding / vello_shaders | 0.9.0（据え置き・既に最新） | 据え置き。最新のまま。 |
| skrifa | 0.42.1（据え置き） | **据え置き**。vello 0.9.0 が `skrifa = "0.42.1"`（= `^0.42.1`）を要求し、0.44 は満たさない。`[patch.crates-io]` は要求を満たす版でしか差し替えられないため、skrifa を 0.44 にすると vello 0.9.0（最新）を壊す。 |
| parley / parley_data / fontique | 0.9.0（据え置き） | **据え置き**。parley 0.11 は新しい skrifa/fontique を要求し、上記の skrifa 制約（vello 0.9.0 が ^0.42 に固定）と両立しない。テキストスタックだけを上げるには vello を最新から降ろす必要があり、駆動する必要（bugfix・機能）も無いため見送る。 |

**理由（互換性マトリクスの結論）:** vello を最新 0.9.0 に保つ限り、それが固定する `skrifa ^0.42.1` / `peniko 0.6.1` により、テキストスタック（skrifa/parley/fontique）は 0.44/0.11 へ進めない（issue #759 が事前に想定した衝突）。テキストスタック更新には具体的な駆動要因が無く、vello を最新から降ろす代償に見合わない。よって **駆動要因のある taffy のみを上げ**、テキストスタックは vello 互換セット（skrifa 0.42.1 / parley・fontique 0.9.0）に据え置く。ADR-0007 の「取り込むかどうかの判断権は Hayate 側」の初適用。

**局所パッチの棚卸し（taffy 0.7.7 → 0.12.1 で撤去/維持）:**

- **撤去**: flexbox `ComputeSize` リトライ（wrap 行クランプ・コミット `a025770`）— 0.12.1 で upstream 修正済み。
- **撤去**: grid item のパーセント解決を grid area 基準にする `grid_item.rs` パッチ — 0.12.1 で upstream が同方向（grid area 基準）を採用済み。
- **維持（1 件）**: flexbox `determine_flex_base_size` の cross size クランプ（`width:620 + max-width:...` のように定 cross size が min/max クランプされる場合、クランプ後の幅で main size を測る）。0.12.1 でも未修正のため唯一の局所パッチとして残す。回帰: `flex_percent_max_width.rs`。

### 2026-07（依存一括更新）— vendor スタックを最新安定版へ追従

依存の互換境界を再評価し、`taffy 0.12.2`、`parley / parley_data / fontique 0.11.0`、
`skrifa 0.45.0` を crates.io の同梱ソースから再 vendor した。`parley` の
`complex-scripts` feature を有効にしたため、従来の CJK auto-script 局所パッチは upstream
実装へ置き換わった。taffy の cross-size clamp は 0.12.2 にも未収録で、
`flex_percent_max_width.rs` の回帰2件が再現したため、従来の局所修正を新ソースへ移植して維持する。

Vello は最新安定版 0.9.0 を維持し、テキストスタックとの整合のため manifest の `skrifa` を
`0.45.0` へ合わせた。一方 `wgpu` は Vello 0.9.0 が公式に宣言する `29.0.3` を維持する。
`wgpu 30.0.0` では surface presentation、buffer mapping、surface color-space APIへの独自追従が
必要になり、Velloの全テスト実行でも不安定性が見られたため、30向け互換パッチは撤去した。
この組み合わせは desktop の全workspace/all-targetテスト、Web の Vello・tiny-skia・null
全Wasm build、Android arm64 debug APK buildで検証済み。
