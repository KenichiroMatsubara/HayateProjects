# Tsubame Adapter は hover イベントと Signal ベースのホバー状態を拒否する

ADR-0056 は擬似スタイルを Hayate Render Layer へ移し、`hover-enter` / `hover-leave` のイベント delivery はアプリロジック用に温存した。一方、Tsubame の hello-world はスタイルを `:hover` へ移した後も `hoveredCard` Signal と `onHoverEnter` / `onHoverLeave` でヒント文言を切り替えており、視覚ホバーとアプリ状態の二重管理が残った。ホバーに見える変化は Hayate CSS の `:hover` のみとし、Framework がホバー境界を追跡して Signal や `setStyle` を駆動するパターンは Tsubame Adapter のエラーとする。

## Considered Options

**`hover-enter` / `hover-leave` を JSX prop として残しドキュメントで非推奨にする案を却下。** 型と実行時の両方で拒否しないとデモと実装が再び乖離する。

**Hayate の `poll-events()` から hover delivery 自体を削除する案を却下。** Element Document Runtime のイベント語彙は温存し、拒否は Tsubame Adapter（`tsubame-solid` 等）の境界に限定する。

## Consequences

- `tsubame-solid` の `TsubameProps` から `onHoverEnter` / `onHoverLeave` を削除
- `setProperty` で当該 prop を受け取った場合は開発時エラー（throw）
- 視覚的ホバーは `style` 内の `:hover` / `:active` / `:focus` のみ（ADR-0056）
- ADR-0056 の「イベント通知は温存」は **Hayate Element Layer** に限定。Tsubame Adapter 経由の hover 購読は不可
- ADR-0019 の hover イベントは low-level host / 将来の非 Tsubame クライアント向けに残る
