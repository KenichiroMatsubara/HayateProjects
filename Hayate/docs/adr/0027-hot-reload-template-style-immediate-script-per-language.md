# Hot Reload は <template>/<style> を即時反映し、<script> は言語ごとに扱いを分ける

`<template>` と `<style>` の変更はすべての言語で即時反映する。
`<script>` の変更は TypeScript・Python では即時反映するが、
Rust ではフルリビルド＋ブラウザリロードとなる。

## 根拠

`<template>` と `<style>` は Hayabusa コンパイラが処理する領域であり、
言語ツールチェーン（tsc / cargo 等）を通らない。
変更をコンパイラが差分処理してランタイムに送れば、
Rust バイナリの再コンパイルなしに即時反映できる。

`<script>` は言語ツールチェーンをフルに通るため、
言語ごとのコンパイル速度に依存する。
Rust は incremental build でも数秒〜数分かかるため、
即時反映は現実的でない。

## Considered Options

- **Rust では Hot Reload を一切提供しない**: シンプルだが、
  スタイル調整のたびにフルリビルドが走る開発体験は悪すぎる。
  `<template>` / `<style>` は Rust バイナリと独立して処理できるため、
  この制約は不必要。
- **セクション別・言語別に反映範囲を分ける（採用）**: `<template>` / `<style>` は
  すべての言語で即時反映。`<script>` は言語の再コンパイル速度に従う。
  開発体験とアーキテクチャの現実的な制約のバランスを取る。

## Consequences

- Hayabusa の開発サーバーは変更ファイルのセクションを判定し、
  `<template>` / `<style>` のみの変更ならランタイムに差分を送る
- `<script>` 変更時は言語ウォッチャー（tsc --watch / cargo watch 等）に委譲する
- Rust プロジェクトでも UI のビジュアル調整は高速に行える
