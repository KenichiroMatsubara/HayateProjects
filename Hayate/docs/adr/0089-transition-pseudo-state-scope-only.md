# transition は擬似状態の切替スコープのみに適用する

**Status: accepted**

**Date: 2026-06-13**

## Context

`transition` プロパティをスタイル語彙に追加するにあたり、どのトリガーに対して補間アニメーションを発生させるかを決める必要があった。CSS の `transition` は「任意のスタイル変化」に反応するが、Hayate には複数の変化経路がある:

- **経路 A**: `:hover`/`:active`/`:focus` 擬似状態の切替（ADR-0056 の Render Layer が管理）
- **経路 B**: アプリケーション層からの `setStyle` 直接呼び出し

「CSS と同じく全経路に適用する」案（フル CSS スコープ）も検討した。

## Decision

`transition` は **擬似状態の切替（経路 A）にのみ** 適用する。`setStyle` 直接呼び出し（経路 B）は従来通り即時反映のままとする。

理由:
- 擬似状態の解決は ADR-0056 に従い Render Layer が完全に所有しており、補間の開始・終了タイミングを Render Layer が制御できる。
- `render(timestamp_ms)`（ADR-0032）と `visual_dirty`（ADR-0086）の仕組みが既存インフラとして存在し、新たなタイマー機構を要しない。
- 経路 B に `transition` を適用するには「変化を検出してアニメーションを起動する」コールサイトをアプリケーション全体に波及させる必要があり、スコープが大きすぎる。
- アプリケーション層で制御したいアニメーション（パネルの開閉など）は `setStyle` + フレームループ（`requestAnimationFrame` 相当）で実現できる。

## Consequences

- `:hover`/`:active`/`:focus` の切替時にスムーズな状態遷移（フェード、スケールなど）が宣言的に書ける。
- アプリケーション駆動のアニメーション（例: リスト項目の追加アニメーション）は `transition` で書けない。アプリ側で逐次 `setStyle` を呼ぶ方式になる。
- 将来フル CSS スコープに拡張する場合は本 ADR を改訂する。
