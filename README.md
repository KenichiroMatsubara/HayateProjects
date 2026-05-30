# Tsubame（燕）

> **Tsubame は、JS/TS 向けのレンダラーターゲット基盤である。**

Tsubame はフレームワークではない。

Tsubame が提供するのは **Renderer Protocol**（`IRenderer`）・**DOM Renderer**・**Canvas Renderer** の 3 つである。各 Tsubame Adapter は自身のフレームワーク固有のランタイム（SolidJS の signals / Vue の `@vue/reactivity` / React の Fiber）をそのまま持ち込み、レンダリング先を Tsubame の Renderer Protocol に向け替える。

```
tsubame-solid         tsubame-vue              tsubame-react
（SolidJS runtime    （@vue/reactivity +       （React Fiber +
  solid-js/universal） createRenderer()）        react-reconciler）
        ↓                    ↓                        ↓
              Renderer Protocol (IRenderer)
                    ↓                ↓
             DOM Renderer      Canvas Renderer
                                    ↓
                      Hayate (apply_mutations) → WebGPU
```

## ステータス

✅ **MVP（T1–T6）実装済み** — Renderer Protocol / DOM / Canvas / tsubame-solid / Hello World デモ

| Phase | 内容 | パッケージ |
|-------|------|------------|
| T1 | `IRenderer` + 型定義 | `@tsubame/renderer-protocol` |
| T2 | DOM Renderer | `@tsubame/renderer-dom` |
| T3 | tsubame-solid | `@tsubame/solid` |
| T4 | Canvas Renderer（`apply_mutations` バッチ） | `@tsubame/renderer-canvas` |
| T5 | バインディング検証（node:test） | `packages/renderer-canvas/test/` |
| T6 | DOM / Canvas ワンボタン切替デモ | `examples/hello-world` |

T7（tsubame-vue）・T8（tsubame-react）は未着手。

## パッケージ構成

```
packages/
  renderer-protocol/   ← IRenderer インターフェース定義
  renderer-dom/        ← DOM Renderer（Hayate 不使用、純 JS CSR）
  renderer-canvas/     ← Canvas Renderer（→ Hayate apply_mutations）
  solid/               ← tsubame-solid（solid-js/universal ベース）
  vue/                 ← tsubame-vue（@vue/runtime-core createRenderer ベース）
  react/               ← tsubame-react（react-reconciler ベース）
```

## Renderer の選択

| Renderer | 条件 | 描画 | Hayate |
|----------|------|------|--------|
| DOM Renderer | 開発・軽量ユース | 直接 DOM 操作 | 不使用 |
| Canvas Renderer | GPU 描画が必要 | Hayate → Vello → WebGPU | 必要 |

Adapter コードはどちらの Renderer を使うかを意識しない。Renderer Protocol が差異を吸収する。

## Hayate との関係

Tsubame と Hayate は**完全に独立したリポジトリ**である。結合点は `apply_mutations(ops: Float64Array, styles: Float32Array)` の仕様のみ。Hayabusa・Hayate コアのいずれも Tsubame の存在を知らない。

## クイックスタート

```bash
pnpm install
pnpm run build
pnpm run test          # T5: apply_mutations バインディング
pnpm run dev:hello     # T6: http://localhost:5173
```

`pnpm` が PATH に無い場合: `npx pnpm@11.5.0 install` などで代替できます。

Hello World デモでは同一の `App` コンポーネントを **DOM Renderer** と **Canvas Renderer**（`MockHayate` による 2D Canvas スタブ）で切り替えられます。実 Hayate WASM は不要です。

## ドキュメント

- [設計仕様書](docs/tsubame-spec.md)
- [ドメイン用語集](CONTEXT.md)
- [アーキテクチャ決定記録](docs/adr/)

## ライセンス

[Apache License 2.0](LICENSE)
