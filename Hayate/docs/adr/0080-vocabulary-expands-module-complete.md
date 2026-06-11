# スタイル語彙はレイアウトモジュール単位で完結させる（flexbox 一括追加）

**Status: accepted**

**Date: 2026-06-11**

## Context

閉じた語彙（ADR-0071）に `flexGrow` はあるのに `flexWrap` がなく、デモが「語彙外なので grid で折り返す」というワークアラウンドを抱えていた。プロパティを需要駆動で1個ずつ足すと、この種の直感に反する穴が再生産される。

## Decision

語彙の拡張は**レイアウトモジュール単位の完結**を原則とする。あるモジュールのプロパティを語彙に含めるなら、そのモジュールを一通り使うのに必要なプロパティ群（Taffy が対応する範囲）を揃えて追加する。「語彙にあるモジュールは両レンダラーで一通り動く」をユーザーと LLM への保証とする。

これに従い flexbox モジュールを完結させる: `flex-wrap` / `flex-shrink` / `flex-basis` / `align-self` / `align-content` を spec proto（style_tags）に追加する（ADR-0078 の codegen 経由）。

## Consequences

- wire 形式は既存の型で足りる（enum 新設: flex_wrap / align_self / align_content、f32、dimension）。
- grid の item 配置（`grid-column` / `grid-row` 等）は現状語彙外＝grid モジュールは未完結。grid を本格対応する際は同原則で一括追加する。
- 追加されたプロパティは両レンダラーでのパリティ検証（hayate-css-parity / golden frame）の対象になる。
