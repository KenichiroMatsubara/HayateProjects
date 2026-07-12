# Hayate Current Spec

この文書は現行契約の短い入口である。歴史的経緯や旧案は ADR と archive を参照する。

## 1. Current Focus

現時点の開発優先は Hayabusa ではなく Tsubame である。Hayate は Tsubame から利用される描画基盤として契約を整える。

根拠:
- [`ADR-0040`](/C:/Users/pinara/Desktop/myapps/HayateProjects/Hayate/docs/adr/0040-tsubame-as-renderer-target-not-signal-runtime.md)
- [`ADR-0049`](/C:/Users/pinara/Desktop/myapps/HayateProjects/Hayate/docs/adr/0049-protocol-yaml-single-source.md)
- [`ADR-0051`](/C:/Users/pinara/Desktop/myapps/HayateProjects/Hayate/docs/adr/0051-tsubame-first-development-priority.md)

## 2. External Boundaries

### 2.1 Tsubame boundary

Hayate-Tsubame 間の現行正本は [`proto/spec/`](../../proto/spec/) の JSON 群（npm: `@torimi/hayate-protocol-spec`）である。`apply_mutations(ops: Float64Array, styles: Float32Array, texts: string[])` と `poll_events` の定数・種別・フィールド名はこの spec に従う。

根拠:
- ADR-0049

### 2.2 Hayabusa boundary

Hayabusa は Hayate の外部 WIT 契約を通らず、Rust crate 依存で `hayate-core` に接続する。Hayabusa は長期構想として保持するが、現行の開発優先契約ではない。

根拠:
- ADR-0045
- ADR-0051

### 2.3 Historical WIT status

WIT と `wit-bindgen` は Hayate-Tsubame 間の現行正本ではない。現行仕様で言及する場合は歴史的設計として扱う。

根拠:
- ADR-0049

## 3. Rendering Model

Hayate は `SceneGraph` を保持し、それを `Scene Renderer` へ渡して描画する。`Render Host` は surface 初期化、capability 判定、renderer 切替、資源寿命管理を担う。renderer の採用順と fallback ルールは `Renderer Selection Policy` が決める。

標準語彙:
- 主候補 renderer: Vello
- 標準代替 renderer: `tiny-skia`
- `Backend`: GPU API 層
- `Scene Renderer`: 描画実装層

根拠:
- [`ADR-0050`](/C:/Users/pinara/Desktop/myapps/HayateProjects/Hayate/docs/adr/0050-scene-renderer-selection-architecture.md)

## 4. Platform Responsibilities

IME、入力、surface、クリップボード、アクセシビリティなどのプラットフォーム依存処理は `Platform Adapter` が担う。Hayate Core は adapter 実装詳細を知らない。

根拠:
- [`ADR-0046`](/C:/Users/pinara/Desktop/myapps/HayateProjects/Hayate/docs/adr/0046-scroll-physics-owned-by-platform-adapter.md)
- 既存 platform adapter 系 ADR 群

## 5. Document Map

- 現行仕様: この文書
- 現行語彙: [`CONTEXT.md`](/C:/Users/pinara/Desktop/myapps/HayateProjects/Hayate/CONTEXT.md)
- 旧仕様: [`docs/archive/hayate-spec-v1.1.md`](/C:/Users/pinara/Desktop/myapps/HayateProjects/Hayate/docs/archive/hayate-spec-v1.1.md)
- 判断根拠: [`docs/adr/`](/C:/Users/pinara/Desktop/myapps/HayateProjects/Hayate/docs/adr)
