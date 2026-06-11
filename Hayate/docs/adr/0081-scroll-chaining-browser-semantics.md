# ネスト scroll-view はブラウザ準拠の scroll chaining を採用する

**Status: accepted**

**Date: 2026-06-11**

## Context

Canvas のホイール配送（`apply_wheel_delta`）は最寄り祖先の ScrollView 1つにデルタを適用し、content bounds で clamp した余りを捨てる。ブラウザ（DOM モード）の既定は scroll chaining — 内側のスクローラーが端に達すると残りが外側の祖先へ連鎖する。ネストした scroll-view の意味論がレンダラー間で食い違っていた。

## Decision

ブラウザ準拠の **scroll chaining を Hayate の仕様**とする。ホイールデルタは最寄り祖先 ScrollView から軸ごとに消費し、clamp で消費しきれなかった残デルタを次の祖先 ScrollView へ伝播する（ルートまで）。DOM 系レンダラーはブラウザ既定のままでよい。レンダラー非依存の意味論統一は system-wide ADR-0002 に従う。

## Considered Options

- **内側で打ち止め（現状の Canvas 挙動）**: 実装は単純だが、Web では `overscroll-behavior: contain` という明示的 opt-in で表現される特殊挙動であり、既定にすると Web ユーザーの期待に反する。DOM 側にも contain の付与が必要になる。却下。
- 将来 `overscroll-behavior` を語彙に追加すれば、chaining の opt-out を CSS 標準の形で提供できる（ADR-0080 のモジュール完結原則の対象外の単発プロパティとして検討）。
