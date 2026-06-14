# position は relative/absolute のみ追加し sticky/fixed は除外する

**Status: accepted**

**Date: 2026-06-13**

## Context

バッジの重ね表示、ツールチップ、ポップオーバーなど、通常のフローレイアウトから要素を切り離して親コンテナ基準で配置したいユースケースがある。CSS の `position` プロパティはこれを `static`/`relative`/`absolute`/`fixed`/`sticky` の 5 値で表現する。

Hayate のレイアウトエンジン Taffy（ADR-0004）が対応するのは `Position::Relative`（デフォルト）と `Position::Absolute` のみ。`sticky` と `fixed` は Taffy に実装がない。

ADR-0071（閉じた語彙）と ADR-0083（モジュール完結）の原則より、「宣言した = 両レンダラーで実装済み」を保証する必要がある。

## Decision

`position: relative | absolute` と `top / left / right / bottom` の **5 プロパティ** をモジュールとして追加する。

- `position: relative`（Taffy デフォルト）は現状と同じ挙動。明示的に書けるようにするための追加。
- `position: absolute` は Taffy の `Position::Absolute` に直接マッピング。通常レイアウトフローから外れ、最近傍の `position: relative`（または `absolute`）祖先を基準に配置される。スペースは確保されない。
- `top / left / right / bottom` は Taffy の `inset` フィールドに対応する。`position: absolute` なしで指定しても効果はない。

`position: sticky` は除外する。Taffy がスクロールコンテキストを考慮した sticky 配置を持たないため、Hayate のレイアウトパスで実装する手段がない。

`position: fixed` は除外する。ビューポート基準の絶対配置は Taffy モデルの外にあり、Platform Adapter ごとの特殊対応が必要になる。

## Consequences

- `z-index`（既存語彙）と `position: absolute` を組み合わせることで、バッジや通知の重ね表示が実現できる。
- `sticky` が語彙にないため、スクロールしても残るヘッダーは ScrollView の外側（スクロール領域より上）に配置するレイアウト設計で対応する。
- `fixed` がないため、画面に固定されるフローティングボタンなどは、アプリケーションのルートレイアウトの最外層に `position: absolute` で配置する方式になる。
