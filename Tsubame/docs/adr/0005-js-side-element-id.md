# ElementId は JS 側で採番する

> **用語更新（ADR-0011・2026-06-27）**: 本 ADR の "Canvas Renderer" / `CanvasRenderer` / `@tsubame/renderer-canvas` は **Hayate Renderer** / `HayateRenderer` / `@tsubame/renderer-hayate` に改名された。本文は決定当時の記録として原文のまま。

Canvas Renderer の `createElement` は当初 `element_create(kind)` を WASM に同期呼び出しして戻り値として ElementId を受け取る設計だった（仕様書 §3.3）。しかし JS 側がモノトニックカウンターで id を採番し `element_create(id, kind)` として WASM に通知する設計に変更した。これにより `createElement` を `ops` バッチストリームに乗せられ、JS→WASM 境界呼び出しを完全に排除できる。Hayate 側の `element_create` シグネチャをこの設計に合わせて変更済み。

## 採用した設計

- JS 側がモノトニックカウンターで `ElementId` を採番
- Canvas Renderer は `OP_CREATE` を `ops: Float64Array` に積み、他の mutation と同じバッチで送る
- DOM Renderer は `createElement` 時に JS 側カウンターで id を振り、Map で DOM ノードと対応付ける

## Considered Options

- **WASM から受け取る（旧設計）**: WASM が id の唯一の発行者でシンプルだが、`createElement` のたびに同期 JS→WASM 呼び出しが発生しバッチに乗らない
