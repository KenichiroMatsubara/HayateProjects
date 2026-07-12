# Renderer Protocol + DOM Renderer + Canvas Renderer の 3 層構成とする

> **用語更新（ADR-0011・2026-06-27）**: 本 ADR の "Canvas Renderer" / `CanvasRenderer` / `@tsubame/renderer-canvas` は **Hayate Renderer** / `HayateRenderer` / `@torimi/tsubame-renderer-hayate` に改名された。3 層構成の決定自体は不変。本文は決定当時の記録として原文のまま。

_origin: Hayate ADR-0036_

JS/TS ユーザー向けフレームワーク Tsubame の設計として、Renderer Protocol（`IRenderer`）を境界として DOM Renderer と Canvas Renderer の 2 実装を持つ構成を採用する。

DOM Renderer では Hayate を使わず JS + HTML のみで CSR 成果物を直接生成する（JS→WASM 境界なし）。Canvas Renderer では Signal の変化をフレーム単位で JS Array にバッチ化し、`apply_mutations(batch)` で Hayate に 1回/frame 渡す。共通の element 型（view / text / image / button 等）をモード間で統一し、Adapter コードはどちらの Renderer を使うかを意識しない。

## 採用した設計

```
Renderer Protocol (IRenderer)
       ↙                   ↘
DOM Renderer          Canvas Renderer
（直接 DOM 操作）    （→ Hayate apply_mutations）
```

- element 語彙は Hayate の React Native 語彙（`view` / `text` / `image` / `button` / `text-input` / `scroll-view`）を統一使用
- DOM Renderer では対応する HTML タグにマッピング
- Adapter は `IRenderer` を受け取り、実装を意識しない

## Considered Options

- Virtual DOM + React hooks: React Native と真正面から競合し差別化できない。エコシステム・知名度で劣る
- Signal + React hooks ファサード（Preact Signals 方式）: 「似てるが違う」混乱を生む。中途半端
- Renderer Protocol なし（Adapter が DOM/Canvas を直接分岐）: Canvas 対応コストが Adapter ごとに発生する
