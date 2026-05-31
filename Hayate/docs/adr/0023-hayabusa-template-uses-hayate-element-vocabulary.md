# Hayabusa テンプレートのタグ名は Hayate の element-kind 語彙に揃える

Hayabusa `.hbs` ファイルの `<template>` セクションで使用するタグ名は、
Hayate WIT の `element-kind`（`view` / `text` / `image` / `button` / `text-input` / `scroll-view`）に直接マップされる名前を採用する。
HTML タグ名（`div` / `p` / `h1` 等）は採用しない。

## Considered Options

- **HTML タグ名を採用し、コンパイラがマッピング**: `<div>` → `view`、`<h1>` / `<p>` → `text` 等の変換テーブルをコンパイラが持つ。Web 開発者への親しみやすさは増すが、`<h1>` と `<p>` が同じ `element-kind` に潰れる意味論的損失が生じ、マッピングの網羅性を保証する責務がコンパイラに加わる。
- **Hayate の element-kind に揃える（採用）**: `<view>`, `<text>`, `<button>` 等をそのまま使う。Hayate の語彙と完全一致し、曖昧なマッピングが不要。Flutter / SwiftUI / React Native 経験者には自然な語彙。
- **WIT の element-kind を HTML タグ名に拡張**: 「DOM 互換は設計目標に含まない」という決定（ADR-0009 廃止済み、CONTEXT.md）と衝突するため却下。

## Consequences

- テンプレート作者は `<div>` ではなく `<view>` と書く
- コンパイラはタグ名 → element-kind の変換テーブルを持たない（1:1 対応）
- HTML に慣れた開発者向けのドキュメントで語彙の違いを明示する必要がある
