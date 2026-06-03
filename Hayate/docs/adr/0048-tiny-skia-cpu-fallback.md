# tiny-skia CPU Rendering Backend as WebGPU Fallback

Canvas モード（Vello + WebGPU）が利用できない環境向けに、tiny-skia を CPU レンダリングバックエンドとして追加する。

## Context

Canvas モードは Vello + wgpu (WebGPU) で描画するが、WebGPU は現時点で Chromium 系ブラウザのみ対応しており、Firefox / Safari（< 17.4）/ GPU なし CI 環境では利用できない。HTML Mode は非 WebGPU 環境でのフォールバックだが、ブラウザ CSS レイアウトに委ねるため Canvas モードの Taffy/Parley レイアウトとのピクセル一致は保証されない。

Canvas モードの SceneGraph パイプライン（ElementTree → compute_layout → scene_build → SceneGraph）をそのまま利用し、最終段のラスタライズのみを CPU で行う代替バックエンドが必要である。

## Decision

`tiny-skia`（Rust 製 CPU 2D ラスタライザ、linebender エコシステム）を `backend-tiny-skia` feature flag で追加する。

### アーキテクチャ

```
ElementTree
  → compute_layout (Parley/Taffy)  ← 共通（バックエンド非依存）
  → scene_build::build()           ← 共通
  → SceneGraph                     ← 共通
  → SelectedBackend::render_scene()
      ├─ [backend-vello]      VelloBackend:    SceneGraph → Vello Scene → WebGPU → canvas surface
      └─ [backend-tiny-skia]  TinySkiaBackend: SceneGraph → tiny-skia Pixmap → putImageData → canvas 2D
```

### バックエンド選択

既存のコンパイル時 feature-gated 型エイリアスパターンを維持し、二つの WASM ビルドを生成する。JS 側で WebGPU 可用性を判定し適切なバイナリをロードする。ランタイムディスパッチは導入しない。

### テキストレンダリング

tiny-skia はテキスト描画を持たない。ワークスペースにベンダリング済みの `skrifa` でグリフアウトラインを抽出し、`OutlinePen` → `tiny_skia::PathBuilder` 変換で描画する。SceneGraph の `TextRunData` は pre-positioned glyph ID を持つため、バックエンドはアウトライン取得とパス塗りつぶしのみを担う。

### ピクセル出力

`tiny_skia::Pixmap`（RGBA8 バッファ）→ `CanvasRenderingContext2d.putImageData()` でブラウザ canvas に転送する。

## Considered Options

**ランタイム enum ディスパッチ（単一 WASM）を却下。** Vello+wgpu と tiny-skia+skrifa の両方がバイナリに含まれ WASM サイズが倍増する。既存のゼロコスト抽象化パターンを壊す。

**fontdue によるグリフビットマップラスタライズを却下。** 新規依存が必要。skrifa は既にベンダリング済みでアウトラインベースの高品質描画が可能。

## Consequences

- `backend/tiny_skia.rs` を新規追加し `CanvasBackend` トレイトを実装する。
- `Cargo.toml` に `backend-tiny-skia = ["dep:tiny-skia", "dep:skrifa"]` feature を追加する。
- `hayate-core` への変更はゼロ。`element_renderer.rs` への変更もゼロ。
- 二つの WASM バイナリが生成される（pkg-vello / pkg-tiny-skia）。
- CPU レンダリングのため大規模 UI では WebGPU 版に劣る（既知制限）。
- Firefox / Safari でも Taffy レイアウト + バンドルフォントによる Canvas モード相当の描画が可能になる。
- ADR-0002（wgpu as sole GPU backend）の原則は維持される：tiny-skia は GPU バックエンドではなく CPU フォールバック。
