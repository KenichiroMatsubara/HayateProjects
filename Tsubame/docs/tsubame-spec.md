# Tsubame Current Spec

この文書は現行仕様の短い入口である。Tsubame は archive 化せず、現役仕様書として維持する。

## 1. Definition

Tsubame は JS/TS 向けの renderer target 基盤である。責務は `Renderer Protocol`・`DOM Renderer`・`Canvas Renderer` の 3 つに限る。signal、component model、scheduler は各フレームワークが持ち込む。

根拠:
- [`ADR-0002`](/C:/Users/pinara/Desktop/myapps/HayateProjects/Tsubame/docs/adr/0002-renderer-protocol-dom-canvas.md)
- [`Hayate ADR-0040`](/C:/Users/pinara/Desktop/myapps/HayateProjects/Hayate/docs/adr/0040-tsubame-as-renderer-target-not-signal-runtime.md)

## 2. Boundaries

### 2.1 Adapter boundary

各 adapter は `IRenderer` を通じてのみ描画する。`tsubame-solid`・`tsubame-vue`・`tsubame-react` は、それぞれの既存ランタイムを保持したまま renderer を差し替える。

根拠:
- ADR-0002

### 2.2 Hayate boundary

Tsubame と Hayate の結合点は `apply_mutations(ops: Float64Array, styles: Float32Array, texts: string[])` と `poll_events` 契約である。定数・opcode・style tag・event field 名の正本は Hayate の [`proto/spec/`](../../Hayate/proto/spec/)（`@hayate/protocol-spec`）である。

根拠:
- [`ADR-0003`](/C:/Users/pinara/Desktop/myapps/HayateProjects/Tsubame/docs/adr/0003-apply-mutations-encoding.md)
- [`Hayate ADR-0049`](/C:/Users/pinara/Desktop/myapps/HayateProjects/Hayate/docs/adr/0049-protocol-yaml-single-source.md)

## 3. Renderers

### 3.1 DOM Renderer

ブラウザ DOM へ直接反映する実装。軽量な確認用 renderer として使える。

### 3.2 Canvas Renderer

JS 内でフレーム分の変更を積み、`requestAnimationFrame` ごとに `apply_mutations` を 1回呼ぶ。`ops` と `styles` の意味論は `@hayate/protocol-spec` に従う。

根拠:
- ADR-0003
- Hayate ADR-0049

## 4. Non-Goals

- Tsubame 独自 signal runtime を作ること
- フレームワーク固有 ecosystem を置き換えること
- Hayabusa の JS 版になること

## 5. Document Map

- 現行語彙: [`../CONTEXT.md`](/C:/Users/pinara/Desktop/myapps/HayateProjects/Tsubame/CONTEXT.md)
- 現行仕様: この文書
- 判断根拠: [`adr/`](/C:/Users/pinara/Desktop/myapps/HayateProjects/Tsubame/docs/adr)
