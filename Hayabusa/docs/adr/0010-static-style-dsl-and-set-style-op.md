# Hayabusa スタイルは static のみ・sink/IR に `set_style` op を足す（インライン `style` 属性）

status: accepted

Date: 2026-06-24

## Context

「見せられるデモ」（Todo が画面に出て触れる）には色・レイアウトが要る（pending-decisions P3）。
だが Hayabusa の sink / Template IR には現状スタイルの op が無く、`<style>` DSL のオーサリング面
（プロパティ集合・単位・`:hover` 等の素通し）とスコープ（コンポーネント単位の scoped style を
持つか）は未決だった。CONTEXT.md は「`<style>` は言語非依存の DSL」「Hayate CSS は要素ローカルの
インラインスタイル」と定義するだけで、ADR が無かった。

reactive なスタイル束縛（`{expr}` 駆動の style プロパティ・条件付きクラス）まで一気に入れると
binding 機構に手が入り risk が上がる。初回デモに必要なのは「静的に色と箱を出す」ことだけ。

## Decision

**初回は static style のみ。** reactive style 束縛は一旦禁止し（必要になった段階で別途 ADR）、
sink / Template IR に「静的スタイルを要素へ一度だけセットする」op（`set_style`）を足すに留める。

- **オーサリング面（このスライス）**：要素インラインの `style="k: v; ..."` 属性で書く。これは
  CONTEXT の「Hayate CSS = 要素ローカルのインラインスタイル」へ最短で写り、セレクタ解決を要しない。
  `<style>` ブロック＋セレクタ・scoped style・`:hover` 等の擬似状態は **後続**（スコープの未決は
  そこで決める）。
- **語彙（閉じた部分集合）**：レイアウト（`width`/`height`/`padding`/`margin`/`gap`/`display`/
  `flex-direction`/`align-items`/`justify-content`）と視覚（`background-color`/`color`/`font-size`）。
  単位は `px` / `%` / `auto`、色は `#hex` と少数の名前付き。Hayabusa は **閉じた独自 style 型**
  （`style::StyleProp` 等）を持ち、`HayateSink` が core の `StyleProp` へ写す（`ElementKind` と同流儀・
  既定ビルドは外部依存ゼロ・ADR-0006）。
- **適用モデル**：`TemplateNode::style: Vec<StyleProp>` を instantiate 時に **一度だけ**
  `ElementSink::set_style` で適用する（`bind_text` のような Effect は張らない）。`set_style` は
  `hayate_core::element_set_style`（要素ローカルインラインスタイル）へ写る。
- **codegen（ADR-0008）**：`.hybs` の `style="..."` を build 時に `StyleProp::...` の Rust へ
  コンパイルする（cargo が型検査）。未知のプロパティ・値・単位は codegen エラーにする。

## Considered Options

- **インライン `style` 属性 ＋ static（採用）**：最短で「色と箱」が出る。セレクタ解決・スコープの
  未決に踏み込まず、Hayate CSS（要素ローカルインライン）へ 1:1。reactive の risk を負わない。
- **`<style>` ブロック ＋ セレクタ解決**：CSS らしいオーサリングだが、セレクタエンジンと scoped
  style の設計（P3 の未決）を今要求する。tracer bullet には過剰。後続へ。
- **reactive style 束縛**：`{expr}` で色・サイズを駆動。binding 機構に手が入り、初回デモに不要。
  必要時に別 ADR（既存 binding に乗る見込みで低 risk）。

## Consequences

- sink trait に `set_style(id, &[StyleProp])` ＋ `Mutation::SetStyle` を追加。`SetStyle` が f32 を
  含むため `Mutation` は `Eq` を落とし `PartialEq` のみになる（assert_eq には十分）。
- Hayabusa の `style::StyleProp` は core 語彙の **部分集合**。新プロパティが要るたびに Hayabusa 型と
  `HayateSink` の写像を一緒に広げる（二重管理だが小さく、ドリフトは写像関数 1 箇所に閉じる）。
- static なので signal 変化でスタイルは再適用されない（`tests/style.rs` で回帰）。reactive style が
  要るときは別 ADR で `bind_*` 同型の経路を足す。
- `.hybs` の `style` 値の誤りは build 時（codegen）に落ちる。生成された `StyleProp::...` は cargo が
  型検査する（ADR-0008 の「型検査される生成 Rust」と整合）。
- 実 layout への反映は `tests/app_host.rs`（`element_layout_rect` 読み戻し）で実証済み。

## 関係

- ADR-0002：host-ABI 線。`set_style` は `element_set_style` へ 1:1 で写る mutation op。
- ADR-0006：self-contained。`style::StyleProp` は閉じた Hayabusa 語彙で、core 写像は `HayateSink`
  （`feature = "hayate-core"`）に閉じる。
- ADR-0008：`.hybs` build 時 codegen。`style="..."` を型検査される生成 Rust へコンパイルする。
- pending-decisions P3：本 ADR が同項（static style と `set_style` 拡張）を決着させる。reactive
  style・`<style>` セレクタ・scoped style は後続の別 ADR。
