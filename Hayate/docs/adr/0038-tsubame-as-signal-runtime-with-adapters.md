# Tsubame をフレームワークではなく Signal ランタイムとし、Adapter 群をその上に構築する

superseded by: ADR-0040

Tsubame は当初「SolidJS 的 Signal + TSX 記法のフレームワーク」として設計されていた。しかし将来 Vue・Svelte・React など複数の記法を Hayate（GPU Canvas）および DOM の両方に対応させることを考えると、記法ごとに独立した Signal ランタイムを持つ設計では Signal ロジックのコンポーネント間共有ができず、Canvas Renderer の実装コストも記法ごとに発生する。そこで Tsubame を pure JS の Signal ランタイム基盤（`createSignal` / `createEffect` / `createMemo` + Renderer Protocol）に再定義し、各記法は Tsubame の上に乗る Tsubame Adapter として実装する構成に変更した。

## Considered Options

- **一枚岩の Tsubame フレームワーク**（TSX 記法に固定）: 実装は単純だが Vue・React ユーザーへの間口が広がらず、エコシステムが育たない
- **記法ごとに独立した Signal ランタイムを持つ設計**: Vue は `@vue/reactivity`、Svelte は Runes など各フレームワーク固有の reactivity を使う。記法をまたいだコンポーネントロジックの共有が不可能になり、Canvas Renderer の実装も各記法が個別に抱える必要があって工数が掛かる

## 採用した設計

```
tsubame-solid │ tsubame-vue │ tsubame-react   ← Tsubame Adapter（記法層）
                    ↓
         Renderer Protocol (IRenderer)         ← Tsubame が定義する境界
          ↙                  ↘
   DOM Renderer          Canvas Renderer       ← Renderer 実装（一回払い）
  （直接 DOM 操作）    （→ Hayate apply_mutations）
```

- **Signal の統一**: Vue の `ref`/`computed`・React の `useSignal` 等はすべて Tsubame の `createSignal`/`createEffect`/`createMemo` の薄いラッパーとして実装する。これにより、Adapter が異なってもコンポーネントのビジネスロジックをそのまま共有できる
- **Canvas Renderer は一回払い**: Renderer Protocol を実装する Canvas Renderer を一度作れば、すべての Tsubame Adapter が自動的に Hayate（GPU Canvas）対応になる。記法ごとの Canvas 対応コストはゼロ
- **実装順**: tsubame-solid（旧 Tsubame JSX 層の引き継ぎ）→ tsubame-vue（開発者数・実装コストのバランス最良）→ tsubame-react（開発者数最多だが競合多・Signal 化前提）
- **tsubame-svelte 除外**: Svelte の価値の大半はコンパイラ最適化と `.svelte` 構文にある。コンパイラ改造の工数に対してメリットが薄いためスコープ外とし、Svelte ユーザーには tsubame-vue を推奨する
- **tsubame-vue のコンパイラ**: `.vue` SFC を採用し、`@vue/compiler-dom` のコードジェネレータ部分を差し替えて Renderer Protocol 呼び出しに変換する
- **tsubame-react の API**: hooks 互換ではなく signal ファースト（`useSignal` / `useComputed`）。互換層の複雑さを避け、「React の JSX 感覚で Signal を書く」という明快なポジションを取る
- **リポジトリ構成**: Tsubame は Hayate とは完全に独立した別リポジトリ（pure JS モノレポ）。結合点は `apply_mutations` の仕様のみ。ADR-0035 の独立原則を継承する
