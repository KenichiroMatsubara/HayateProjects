# Hayabusa はモノレポ内の Hayabusa/ クレートとして配置し、hot-reload は reconcile の特殊ケースとする

status: accepted
supersedes: Hayate ADR-0025（独立リポジトリ + hayabusa:runtime WIT）
refines: Hayate ADR-0027（hot-reload の機構を具体化）

## 配置：独立リポジトリではなくモノレポ内クレート

Hayabusa は独立リポジトリを持たず、モノレポ内の **`Hayabusa/` クレート**として配置する
（`Hayate/`・`Tsubame/` と並ぶ兄弟）。`hayate-core` に **path 依存・一方向**でリンクし、
WIT 境界は持たない（ADR-0001 / Hayate ADR-0045 通り）。

これは Hayate ADR-0025（独立リポジトリ＋`hayabusa:runtime` WIT 定義）を **supersede** する。
0025 が前提とした独立リポジトリは root ADR-0001（モノレポ統合）により無効化され、
WIT 境界は Hayate ADR-0045 により既に撤廃されている。一方向依存（Hayate は Hayabusa を
知らない）という設計原則自体は path 依存で物理的に保たれる。

ADR の配置：Hayabusa 固有 ADR は **`Hayabusa/docs/adr/`** に置く（CONTEXT-MAP.md に追記）。
過去の Hayabusa ADR（`Hayate/docs/adr/` の 0023–0027, 0035, 0045）は歴史的経緯で
Hayate ツリーに同居しているが、本ディレクトリが以降の権威ソースとなる。

## hot-reload は reconcile の特殊ケース

Hayate ADR-0027 の「`<template>` / `<style>` は即時反映」を、**reconcile の特殊ケース**として
具体化する（refine）：

- template / style の編集 ＝ **新しい Template IR への keyed reconcile**（ADR-0004）
- signal の状態は **Scope identity** で保存される（`count` がリセットされない）
- 専用のスナップショット＆復元機構は作らない（reconcile を再利用する）

全 wasm 方向（ADR-0001）では `<script>` 変更は言語ツールチェーン（wasm コンパイル）を
通るため、即時反映は当該ゲスト次第になる（0027 の「TS / Py は即時」という前提は、
インタプリタ埋め込みを廃したことで当該ゲストの性質に依存する形に更新される）。

## 優先度

Hayate ADR-0051（Tsubame-first development priority）と**並行**する。本 ADR 群は
0051 を supersede しない（Tsubame の継続と Hayabusa の着手は両立する）。

## 最初の実装（tracer bullet）

カウンタ例を `Hayabusa/` クレートで実装する：自作 fine-grained コア＋最小式評価器＋
手組み Template IR で、`count` signal・`{count}` 束縛の `<text>`・increment する
`<button>` を instantiate → bind → fine-grained patch → ElementTree 駆動し、
**テキストノードだけが patch される**ことをテストで実証する。`.hybs` / コンパイラ /
他言語 / router 等は含めない。

## Consequences

- Hayabusa のコードと ADR がモノレポ内の `Hayabusa/` に一元化される
- hot-reload は別サブシステムにならず reconcile に吸収される
- Tsubame の開発を止めずに Hayabusa の最初の slice に着手できる
