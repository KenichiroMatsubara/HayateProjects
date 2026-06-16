# コードポイント→フォールバックファミリをデータ駆動の coverage テーブルに統一する

**Status: accepted**

**Date: 2026-06-16**

## Context

ADR-0042 は「`.notdef` 検出 → `codepoint_font_family(cp)` でファミリ名へ変換 →
`FetchFont{family}` 発火」という on-demand フォント調達経路を定め、ADR-0043 が
「ファミリ名 → URL（調達）はアダプタ所有」と層を分けた。`codepoint_font_family`
は core 所有の「Unicode ブロックテーブル」である。

この `codepoint_font_family` は **スクリプトごとに 1 アームを手書きする `match`**
として実装されていた。これが次の実問題を生んでいた:

- **網羅性の欠落**: emoji を追加した際、`U+1F300..1F6FF` 等の主要ブロックだけを
  足したため、国旗（Regional Indicator `U+1F1E6..1F1FF`）・トランプ（`U+1F0A0..`）・
  麻雀（`U+1F000..`）・`U+2B50 ⭐` 等が漏れた。「🌙 は直ったが次は 🇯🇵 が空白」という
  モグラ叩きになる（issue #329 のレビュー指摘）。
- **拡張がコード変更**: 新スクリプト／新ブロックのたびに core の `match` を手書きする。
- **層をまたぐ整合が無検証**: core が返すファミリ名にアダプタの manifest が URL を
  与えているか、を保証する仕組みが無く、`FetchFont` が dead-end しても気付けない。

ADR-0061 は既に同種の問題（手書きリストの drift）を「JSON manifest を正本にして
coverage check で検証」する方針で解いている。`codepoint_font_family` はその方針が
未適用に残った最後の手書きフォントテーブルだった。

## Decision

`codepoint_font_family` の手書き `match` を廃し、**単一のソート済み coverage テーブル**
に統一する。ADR-0042/0043 の層（codepoint→family は core、family→source はアダプタ）は
維持する。

### 1. `font_coverage` モジュール（core, platform 非依存）

`crates/core/src/element/font_coverage.rs` に:

- `pub const FONT_COVERAGE: &[Coverage]` — `{start, end, family}` の **start 昇順・
  非重複**な配列。全スクリプト（CJK/ハングル/アラビア/タイ/デーヴァナーガリー/
  ヘブライ）と emoji を 1 枚に集約。
- `pub fn family_for_codepoint(cp) -> Option<&'static str>` — `partition_point` による
  二分探索。`None` は「バンドル既定フォントが持つはず」を意味する。
- `pub fn coverage_families() -> Vec<&'static str>` — ルーティング先ファミリ集合。
  アダプタの整合検証に使う。

不変条件（ソート済み・非重複）は `table_is_sorted_and_non_overlapping` テストで担保。

### 2. emoji は「ブロック丸ごと」で網羅する

ルーティングは呼び出し側（`text::lower_glyph_runs`）で **`.notdef` ゲート済み**である
ため、emoji レンジは広めに取ってよい（基底フォントが持つグリフは `.notdef` にならず
そもそも `family_for_codepoint` に到達しない）。この性質を使い、symbol 面を丸ごと
`Noto Emoji` に向ける:

- `U+2600..27BF`（Misc Symbols + Dingbats）
- `U+2B00..2BFF`（Misc Symbols and Arrows）
- `U+1F000..1FAFF`（麻雀・ドミノ・トランプ・囲み英数（**国旗含む**）・各種絵文字・
  Emoticons・Transport・Supplemental・Extended-A）

これで「現在および将来のあらゆる絵文字」を、レンジ追加なしで（既存ブロック内の新規
割当は自動的に）カバーする。

### 3. 補完先はモノクロ `Noto Emoji`（カラーではない）

tiny-skia の painter は glyf/CFF アウトラインのみ描画し COLR/CBDT を描けない。よって
emoji の補完先は **モノクロ `Noto Emoji`** とする。`Noto Color Emoji` ではカラー
ビルドのため tiny-skia 下では依然空白になる。

### 4. 層をまたぐ整合チェック（adapter）

`coverage_families()` の各ファミリに、web adapter の `fonts.json`（ADR-0061）が URL を
与えていることを `hayate-adapter-web` のテストで検証する
（`every_coverage_family_is_procurable`）。これにより **coverage テーブルをデータだけで
安全に拡張**できる: 調達手段の無いファミリへルーティングすると CI が落ちる。

## Considered Options

- **build.rs で JSON manifest から codegen（ADR-0061 完全踏襲）**: core に build.rs と
  `serde_json` の build-dependency を増やす。coverage は URL と異なり重複正本が無い
  （ルーティング表は 1 枚）ため、drift 解消目的の codegen は不要。`&'static str` 契約
  （`missing_families: Vec<&'static str>`）を保つには const テーブルが素直。→ 不採用。
- **Unicode `Emoji` プロパティでの動的判定**: `unicode-properties` 等の依存追加が必要で、
  `Emoji` プロパティは `#`/`*`/数字など text-default も含み過剰。`.notdef` ゲート下では
  ブロック単位の粗いレンジで十分。→ 不採用。
- **`.notdef` 時にフォントの cmap を実探索**: on-demand フォントは取得前で cmap が無い
  （鶏卵問題）。ブラウザ同様、事前計算した coverage 表が現実解。→ 不採用。
- **narrow な emoji レンジ追加のまま**: 網羅性が無く同種報告が再発。→ 不採用（本 ADR の
  動機そのもの）。

## Consequences

- core の `codepoint_font_family` 手書き `match` を削除し `font_coverage` に統一。
  `lower_glyph_runs` は `font_coverage::family_for_codepoint` を呼ぶ。
- emoji は主要ブロックを丸ごとカバーし、国旗・トランプ等の漏れが解消。
- 新フォント／新スクリプトの追加は **`FONT_COVERAGE` への 1 行 ＋ `fonts.json` への
  URL** で済み、整合は CI が保証する。
- 関連: ADR-0042（検出点と層）、ADR-0043（URL はアダプタ）、ADR-0061（manifest と
  coverage check）、ADR-0073（バンドル既定フォント）。
- スコープ外（issue #332）: 本 ADR は「正しいモノクロフォントを確実に取得する」層に
  限る。**カラー絵文字（COLR/CBDT）は Vello（WebGPU）なら描画可能**
  （`crates/vendor/vello/src/scene.rs` の `draw_glyphs().draw()` が COLR/CPAL・bitmap を
  検出し `try_draw_colr` へ分岐）だが、CPU フォールバックの tiny-skia は `outline_glyphs()`
  のみで COLR/CBDT を描けない。共通 routing が単一ファミリを返す現設計では最小公倍数の
  モノクロに縮退する。Vello 限定でカラー化する（レンダラ別フォント出し分け）作業は
  adapter 層（ADR-0043）の責務として #332 に分離した。
