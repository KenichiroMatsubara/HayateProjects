# Hayabusa は hayate-core へクロスワークスペース path 依存でリンクする（patch 複製＋optional feature）

status: accepted

Date: 2026-06-23

## Context

Hayabusa の `ElementSink`（`src/sink.rs`）は設計上 `hayate_core::ElementTree` の対応 API に
1:1 で写る host-ABI 線（ADR-0002）で、実コアを駆動する `HayateSink` は「薄い後続実装」と
位置づけてきた（README / pending-decisions P1）。だがその実装に入る前に、**ビルド現実**の
未検証ブロッカーが残っていた（pending-decisions P1・「ADR-0117 はこのビルド現実に触れていない」）：

- Hayabusa は Hayate ワークスペースとは**別ワークスペース**（独立した `Cargo.toml` / `Cargo.lock`）。
- hayate-core は `vello` / `taffy` / `parley` / `fontique` / `skrifa` 等を vendored crate に差し替える
  `[patch.crates-io]` を **Hayate ワークスペースの** `Cargo.toml` に持つ（ADR-0007）。
- cargo は依存先ワークスペースの `[patch]` を**継承しない**。別ワークスペースから hayate-core を
  bare path 依存で引くと patch が効かず、vendored でない crates.io 版（例：`fontique-0.9.0`）を
  解決してコンパイルが失敗する可能性があった。

この一点が「実装ブロッカー（spike 対象）」として明示され、**spike を先に行う**ことが推奨されていた。

## Decision

**spike を実施し、結果として次の構成でリンクする。**

1. **patch テーブルを Hayabusa 側に複製する。** `Hayabusa/Cargo.toml` に `[patch.crates-io]` を置き、
   Hayate と同じ vendored crate（`../Hayate/crates/vendor/*`）を指す。これでクロスワークスペースの
   依存解決が vendored 版に揃い、リンクが通る。
2. **hayate-core は optional dependency ＋ feature でゲートする。** 既定ビルドは外部依存ゼロの
   self-contained（ADR-0006）を保ち、`feature = "hayate-core"` を有効化したときだけ実コアと
   `HayateSink`（`src/hayate_sink.rs`）をコンパイルする。

### spike の結果

- **bare path 依存（patch なし）→ 失敗**：crates.io の `fontique-0.9.0` を解決し、vendored fontique
  の API（fontconfig-dlopen 系シンボル）と不一致でコンパイルエラー。P1 の懸念を実証。
- **patch 複製あり → 成功**：hayate-core が Hayabusa からビルドでき、counter tracer bullet を実機
  `ElementTree` 上で駆動する統合テスト（`tests/hayate_sink.rs`）が緑。
- **既定ビルド（feature off）**：`[patch]` は graph に乗らず inert。cargo が
  「patch ... was not used in the crate graph」を**警告**するが（feature off の optional dep で起こる
  既知の挙動・cargo 自身が help でそう説明する）、ビルド・テスト・clippy は通る。

## Considered Options

- **patch 複製 ＋ optional feature（採用）**：別ワークスペースのまま最小の追加で繋がる。既定の
  self-contained 性（ADR-0006）を壊さず、実コア統合は opt-in。欠点は patch テーブルの二重管理と、
  feature off 時の未使用 patch 警告。
- **Hayabusa を Hayate ワークスペースのメンバーにする**：patch を自動継承でき複製不要。ただし
  Hayabusa の独立した ADR コンテキスト・独立 `Cargo.lock`・self-contained tracer-bullet 方針
  （ADR-0006）を崩し、Hayate のフル依存（wgpu / vello 等）が常に Hayabusa のビルドに乗る。今段階の
  「逆張りで単独所有」姿勢に反するため却下。
- **vendored crate を crates.io へ公開して patch を消す**：上流切断（ADR-0007）の意図に反し、射程外。

## Consequences

- `HayateSink`（`src/hayate_sink.rs`）が `ElementSink` を実 `ElementTree` へ 1:1 転送する。
  `ElId(n) ↔ ElementId::from_u64(n)` の全単射で要素 id を写し、id は sink が単調増加で払い出す。
- 実コア統合テストで判明した**意味論差の記録**：`hayate_core` の `element_set_text` は text-like 要素
  （`Text` / `TextInput`）のみに適用し、`Button` への set は no-op（buttons はラベルを子 `text` 要素で
  持つ）。tracer-bullet の `RecordingSink` は kind を問わず記録するため差が出る。Button ラベルを実コアで
  出すには子 text ノードを置く必要があり、これは後続テンプレ／`.hybs` codegen（ADR-0008）の責務。
- patch テーブルは Hayate 側（ADR-0007）と二重管理になる。vendored 構成が変わったら両方を更新する。
- feature off の既定ビルドで未使用 patch 警告が出るのは許容する（cargo の制約で `[patch]` は feature
  条件付きにできない）。`-D warnings`（rustc lint 対象）は通る。
- これで pending-decisions P1 の spike が解消。残りは実装タスク（App Host の `&mut ElementTree` を
  借りる `DeliverySink` 経路＝ADR-0117 の borrowed-tree モデルと、ListenerId → handler ルーティング）。

## 関係

- ADR-0002：host-ABI 線。`ElementSink` の 1:1 写像を実コアで成立させたのが本 ADR の `HayateSink`。
- ADR-0006：self-contained tracer bullet。実コア統合を optional feature にして既定の外部依存ゼロを維持。
- ADR-0007（Hayate）：vendored crate と `[patch.crates-io]`。本 ADR はその patch を Hayabusa 側へ複製する。
- ADR-0117（Hayate）：App Host boot シーム。`HayateSink` を App Host の `DeliverySink` として
  `&mut ElementTree` 借用モデルへ載せる event-loop 配線は本 ADR の次段（実装タスク）。
- pending-decisions P1：本 ADR が同項の「残る未決（ビルド spike）」を解消する。
