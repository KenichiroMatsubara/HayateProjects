# style DSL の enum 語彙は Hayate の proto/spec を正本として `hayabusa-style-vocab` が生成する

status: accepted

Date: 2026-07-03

## Context

`src/style.rs` の enum（`Display` / `FlexDirection` / `Align` / `Justify`）と
`codegen/src/lib.rs` の `style_enum`（キーワード文字列 → variant 名の match）は、同じ閉じた
語彙（`"flex"` → `Display::Flex` 等）を独立に手書きしていた。ズレを自動で防ぐ手段はなく、
「codegen が実在しない variant 名を吐けば rustc がコンパイルエラーで気づく」「`style.rs` に
variant を足してもキーワードを足し忘れれば `.hybs` 側が `unsupported value` エラーで気づく」
という**検出**に頼っていた（アーキテクチャレビュー・2026-07-03）。

一方、この語彙自体は Hayabusa 固有のものではなく、Hayate CSS の語彙として
`Hayate/proto/spec/enums.json`（キーワード集合）と `style_tags.json`（プロパティ名 ↔ enum 名の
対応）に既に正本がある。`Hayate/proto/generator` と `Tsubame/proto/generator` は同じ spec から
Rust / TypeScript それぞれの語彙を生成しており（意味論パリティ・CONTEXT.md）、Hayabusa だけが
この語彙のコピーを独自に手書きし続ける理由はない。

ただし Hayabusa の既定ビルド（`.hybs` の build-time codegen。`hayate-core` feature の有無に
関わらず常に走る）はこれまで `Hayate/` 側を一切参照せず、`Hayate/` への依存は
`hayate-core` feature を有効化したときの実行時クレートリンクに限定されていた（ADR-0006 /
ADR-0009）。spec ファイルの参照は今回が初めて。

## Decision

- 新クレート `Hayabusa/style-vocab/`（`hayabusa-style-vocab`）を `codegen/` の兄弟として追加する。
  `hayabusa-codegen` 同様 pure-std・依存ゼロ（ビルド依存として使われうるため）。
- このクレートの `build.rs` が `../Hayate/proto/spec/enums.json` と `style_tags.json` を自前の
  最小限 JSON パーサ（serde 等には依存しない）で読み、Hayabusa が対応する4つの style tag
  （`display` / `flex-direction` / `align-items` / `justify-content`。ADR-0010 のスコープ）だけを
  フィルタする。`cargo:rerun-if-changed` を両ファイルに張る。
- キーワードは snake_case（spec）→ kebab-case へ機械的に変換する（Tsubame の generator と同じ
  規則）。Hayabusa 独自に持っていた `"start"` / `"end"` の裸エイリアス（`flex-start` /
  `flex-end` の省略形）は廃止し、Tsubame・Hayate CSS と語彙を完全一致させる。
- 変換結果を `pub const ENUM_KEYWORDS: &[...]`（keyword ↔ variant 名 ↔ プロパティ名の3つ組）
  として `OUT_DIR` に生成し `include!` する。
- `hayabusa` 本体の `build.rs` は `hayabusa-style-vocab` を build-dependency として使い、
  `style.rs` の `Display` / `FlexDirection` / `Align` / `Justify` enum 自体をここから生成する
  （`include!`。`Length` / `Rgba` は非 enum なので手書きのまま）。既存の `generated::` コンポーネント
  （ADR-0008）と同型のパターン。
- `hayabusa-codegen` は `hayabusa-style-vocab` を通常の `[dependencies]` として使い、
  `style_enum` の match 文をテーブル参照に置き換える。
- 既定ビルドが `Hayate/proto/spec/*.json` をコンパイル時ソースとして参照することを許容する。
  これは ADR-0006 / ADR-0009 が線引きした「`hayate-core` への実行時クレートリンク」とは別種の
  結合（specファイル参照 ≠ クレート依存）であり、Hayabusa がモノレポ内クレートである前提
  （ADR-0006 が独立リポジトリ案 ADR-0025 を supersede した理由そのもの）の上でのみ成立する。

## Considered Options

- **Hayabusa 独自の手書きデータテーブルを新設**：却下。Hayate CSS の語彙のコピーをもう1つ
  増やすだけで、正本は Hayate 側にあるという前提（CONTEXT.md）に反する。
- **宣言的マクロで `style.rs` の enum をその場生成**：却下。`macro_rules!` は別クレートの
  `const` 値を読めないため、`hayabusa-codegen` 側と語彙を共有する経路にならない。
- **`Hayate/proto/generator` を拡張して Hayabusa 向け成果物を `proto/generated/` に出す**：
  見送り。Hayate 側の生成パイプラインへの変更を伴い、今回のスコープを超える。将来 Hayabusa が
  spec 駆動の対象プロパティを増やす段になったら再検討する。

## Consequences

- `style.rs` と `codegen` 間の語彙の二重管理が構造的に解消される（検出ではなく共通化）。
- Hayabusa の既定ビルドは、モノレポ内で `Hayate/` が兄弟ディレクトリとして存在することを
  前提にする。独立リポジトリ化（旧 ADR-0025、ADR-0006 が却下済み）とは以後も両立しない。
- `.hybs` の `<style>` で `align-items: start` のような裸エイリアスが使えなくなる
  （`flex-start` のみ）。Tsubame・Hayate CSS と語彙が一致する破壊的変更。
- スコープは現在対応する4つの enum 系プロパティに限定。色名・プロパティ名（`width` 等）は
  対象外（ADR-0010 のまま、単一の場所にしかない知識であり二重管理ではないため）。

## 関係

- ADR-0006：既定ビルドの自己完結性。本 ADR は「実行時クレートリンク」ではなく「コンパイル時
  spec ファイル参照」を新たに許容する形でこれを補う。
- ADR-0009：hayate-core へのクロスワークスペースリンク。本 ADR とは異なる種類の結合
  （spec ファイル参照 vs クレートリンク）。
- ADR-0010：static style DSL のスコープ（本 ADR が対象とする4 props の由来）。
- Hayate CONTEXT.md「Style Channel」「意味論パリティ」：proto/spec が正本であるという原則を
  Hayabusa にも適用した。
