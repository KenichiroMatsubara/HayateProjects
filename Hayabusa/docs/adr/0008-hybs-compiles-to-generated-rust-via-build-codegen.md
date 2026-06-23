# `.hybs` は build 時 codegen で生成 Rust にコンパイルする（Rust-native script）

status: accepted

Date: 2026-06-23

## Context

初回デモ（Todo）は、テストのような手組み Template IR ＋ Rust ハンドラではなく、**`.hybs`
ファイルをコンパイルした出力**として動くべき、と決めた。`.hybs` であること自体に意味があり、
オーサリング面（`<template>` / `<style>` / `<script>`）を第一段階から見せる。

`.hybs` の `<script>` は **Rust-native**（ADR-0001 の Rust-first・境界ゼロ）。Rust script は
wasm ゲスト境界（ADR-0002 の flat-array C-ABI）を経由せず、ランタイムへの直接呼び出しに退化する。
したがって `.hybs` の `<script>` は crate と一緒に native コンパイルされる必要があり、「`.hybs` を
どうやって Rust に落として cargo に通すか」というコンパイル機構が未決だった（pending-decisions P6）。

`<template>` / `<style>` マークアップのパーサは ADR-0004 ＋ CONTEXT.md の Template DSL 定義から
低リスクで導出できる。難所は **Rust-native `<script>` のコンパイル/登録経路**だった。

## Decision

**`.hybs` は build 時 codegen で生成 Rust にコンパイルする。** build.rs（または build.rs が起動する
専用 codegen バイナリ）が `.hybs` をパースし、コンポーネントごとに生成 `.rs` を出力する。

- **`<template>`** → Template IR を構築する Rust コードを生成する（要素・`:if`・`:each`・
  コンポーネント・束縛式は既存の `template` / `expr` / `parse` モジュールに落とす）。
- **`<style>`** → static style をセットするコードを生成する（reactive style は当面禁止・P3）。
- **`<script>`（Rust）** → 本体を **setup 関数としてそのまま生成モジュールへ差し込む**。よって
  `<script>` の Rust は cargo に通常どおり型検査され、signal / handler を定義する。`<template>` の
  束縛・`on:click` 等は、`<script>` が定義した名前へ codegen が配線する。

生成物は cargo が通常コンパイルする（`OUT_DIR` へ出力し `include!`、等の具体配置は実装時に確定）。
これにより Rust script は「境界ゼロの直接呼び出し」（ADR-0001/0002）のまま、`.hybs` という単一
ファイル単位（HAYA-04）を保てる。

## Considered Options

- **build.rs / 専用 codegen バイナリ（採用）**：`.hybs` を実ファイルのままパースして生成 Rust を出す。
  「`.hybs` をコンパイルして出力」に忠実で、1ファイル＝1コンポーネント（HAYA-04）を保ち、Rust script
  変更がフルリビルド（HAYA-05）になるのとも整合する。
- **proc-macro（`component!{ ... }` インライン）**：テンプレ等を Rust ソース内にインラインで書く。
  `.hybs` の内容が Rust ソースへ溶け、別ファイル性（HAYA-04）が崩れる。`.hybs` を一次成果物にする
  本デモの方針に合わず却下。
- **手組み Template IR ＋ Rust ハンドラ（テスト同型）**：`.hybs` を介さない。動くものは作れるが
  「`.hybs` をコンパイルした出力」という要求を満たさないため、デモの形としては却下（テストでは継続使用）。

## Consequences

- Hayabusa crate は `.hybs` を入力に取る build 段を持つ。生成 Rust が一次中間生成物になる。
- `<script>` の Rust が生成モジュールに差し込まれるため、型エラー・借用エラーは通常の cargo ビルドで
  そのまま出る（別言語ランタイムのような実行時失敗にならない）。
- 他言語 script（wasm ゲスト・ADR-0001/0002）は本 ADR の射程外。Rust-native の codegen 経路を先に
  確立し、他言語はエンコーダ＋ゲストコンパイルとして後続で別途決める。
- opcode／生成コードの具体レイアウトは tracer bullet 実装で発見・確定する（ADR-0002 と同じ姿勢で
  過度な事前設計をしない）。

## 関係

- ADR-0001：Rust-first・境界ゼロ。Rust script は native 直接呼び出し → 本 ADR は「crate と一緒に
  native コンパイルするための `.hybs`→Rust codegen」を定める。
- ADR-0002：host-ABI の codegen は他言語エンコーダの話。本 ADR の codegen は `.hybs`→Rust SFC
  コンパイルで別レイヤ。
- ADR-0004：Template IR・束縛・式 DSL。`<template>` 生成コードの落とし先。
- HAYA-04 / HAYA-05（spec §12）：1ファイル＝1コンポーネント `.hybs`、Rust は変更でフルリビルド。
