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

🚧 **設計フェーズ** — ADR 確定済み、実装準備中

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

## ドキュメント

- [設計仕様書](docs/tsubame-spec.md)
- [ドメイン用語集](CONTEXT.md)
- [アーキテクチャ決定記録](docs/adr/)

## ライセンス

[Apache License 2.0](LICENSE)
