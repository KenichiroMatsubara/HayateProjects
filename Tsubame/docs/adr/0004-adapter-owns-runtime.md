# Tsubame をレンダラーターゲット基盤とし、各 Adapter が既存ランタイムを持ち込む

> **用語更新（ADR-0011・2026-06-27）**: 本 ADR の "Canvas Renderer" / `CanvasRenderer` / `@tsubame/renderer-canvas` は **Hayate Renderer** / `HayateRenderer` / `@tsubame/renderer-hayate` に改名された。本文は決定当時の記録として原文のまま。

_origin: Hayate ADR-0040（ADR-0038 を supersede）_

ADR-0038 では Tsubame を unified signal ランタイムと定義し、tsubame-vue / tsubame-react の reactivity を Tsubame の `createSignal`/`createEffect`/`createMemo` のラッパーとして実装する方針を採用した。しかし「UI コンポーネント（マークアップを含む）は記法が異なるため adapter をまたいで共有できない」という事実と突き合わせると、signal 統一の主要なメリットが失われる。加えて、signal 統一は Vue・React の 3rd party エコシステム（Pinia / TanStack Query / Zustand 等）を全滅させるコストを伴う。そのため Tsubame の責務をレンダラーターゲット層（Renderer Protocol + DOM Renderer + Canvas Renderer）に限定し、各 adapter は既存ランタイムをそのまま持ち込む設計に変更する。

## 採用した設計

```
tsubame-solid         tsubame-vue              tsubame-react
（SolidJS runtime    （@vue/reactivity +       （React Fiber +
  solid-js/universal） createRenderer()）        react-reconciler）
        ↓                    ↓                        ↓
              Renderer Protocol (IRenderer)
                    ↓                ↓
             DOM Renderer      Canvas Renderer
                                    ↓
                           Hayate (apply_mutations)
```

- `tsubame-solid`: SolidJS の `solid-js/universal` でレンダリング先を Renderer Protocol に向け替える
- `tsubame-vue`: `@vue/runtime-core` の `createRenderer()` でレンダリング先を Renderer Protocol に向け替える。`@vue/reactivity` はそのまま使う
- `tsubame-react`: `react-reconciler` でレンダリング先を Renderer Protocol に向け替える。React Fiber はそのまま使う

## なぜ ADR-0038 を覆したか

ADR-0038 の signal 統一がもたらすとされた主なメリットは「adapter をまたいだコンポーネント共有」だった。しかし UI コンポーネント（マークアップを含む）は記法が異なるため定義上共有不可能であり、共有できるのはヘッドレスロジック（store・computed）のみである。これは現在の Vue/React エコシステムで composable を別フレームワークで使えないのと同じ制約であり、signal を統一しても解決しない。

一方でコストは大きい。Vue ユーザーが移行する際に Pinia / VueUse / VueRouter が動かない、React ユーザーが移行する際に TanStack Query / Zustand が動かない状態では、「既存コードを最小変更で Hayate に対応させる」という訴求力が消える。

レンダラーターゲット設計は React Native・NativeScript-Vue・SolidJS Universal と同じ実証済みパターンであり、各フレームワークのエコシステムの引力をそのまま利用できる。

## Considered Options

- **一枚岩の Tsubame フレームワーク**（TSX 記法に固定）: 実装は単純だが Vue・React ユーザーへの間口が広がらず、エコシステムが育たない
- **Signal ランタイム統一**（ADR-0038）: 上記の理由で廃棄。エコシステム全滅コストに見合うメリットがない
