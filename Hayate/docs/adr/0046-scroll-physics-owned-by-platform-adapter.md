# スクロール物理演算は Platform Adapter が担い、scroll イベントはアプリ通知専用とする

status: accepted  
supersedes: ADR-0022

## Context

ADR-0022 は「scroll offset の状態を Hayabusa（上位層）が管理し、毎フレーム `element_set_scroll_offset` で Hayate に渡す」方式を採用した。

しかし実装を進める中で、この設計には以下の問題が明らかになった。

1. **主要プラットフォームとの逆行**: Web（ブラウザ）・iOS（UIScrollView）・Android（OverScroller）はいずれも「下層がスクロール物理を保持し、アプリ層にはイベントだけ上げる」構造を採る。ADR-0022 の設計はこれと逆方向である。
2. **コードと ADR の乖離**: Canvas Mode の `on_wheel` 実装はすでに Platform Adapter が直接 `element_set_scroll_offset` を呼んでおり、Hayabusa を経由していない。ADR と実装が矛盾していた。
3. **スクロール用途の分離**: scroll イベントは「offset 積算のため」ではなく「parallax・lazy load トリガー・アプリ固有 UI 更新」のために上位層が必要とするものであり、これらは別概念である。

## Decision

**Platform Adapter がスクロール物理演算（delta 積算・イナーシャ・rubber-band・スナップ）を担う。**

- Platform Adapter は wheel イベント・タッチジェスチャーを受け取り、スクロール物理を自己処理して `element_set_scroll_offset` を Hayate に渡す。Hayabusa を経由しない。
- `scroll` イベントはアプリへの通知専用とする。Hayabusa は受け取るかどうかを自由に選べ、offset 積算目的には使わない。
- スクロール挙動の設定（スナップ・ページネーション等）は `scroll-view` element の Hayate CSS プロパティ（`scroll-snap-type` / `scroll-snap-align` 等）で宣言する。Platform Adapter は Element Layer 経由でこれを読んで物理演算に反映する。WIT に専用 API を追加しない。
- `element_set_scroll_offset` WIT API は残す。用途は**プログラマティックスクロール専用**（スクロールトップボタン・特定アイテムへのジャンプ等）。

## Considered Options

- **ADR-0022 継続（Hayabusa 管理）**: scroll イベントで delta を上げ、Hayabusa が積算して毎フレーム `element_set_scroll_offset` を呼ぶ。イナーシャは Hayabusa のライブラリ層で実装。すべての主要プラットフォームの設計と逆行しており、Canvas Mode 実装とも乖離していたため却下。
- **Hayate Core 管理**: Hayate が scroll offset を内部で積算する。ADR-0022 が「Hayate 保持方式」として却下した理由（ADR-0018 poll モデルとの不整合・プラットフォーム差異の吸収困難）は依然有効。却下。
- **Platform Adapter 管理（採用）**: 各プラットフォーム（Web Canvas Mode・iOS・Android・Desktop）の Adapter がそのプラットフォームのスクロール慣習を実装する。Web は wheel delta を即時適用、iOS は UIScrollView 相当のジェスチャー処理、Android は OverScroller 相当。Hayate Core は変わらず「渡された offset でクリップして描く機械」に徹する。

## Consequences

- Canvas Mode の `on_wheel` が Platform Adapter で `element_set_scroll_offset` を直接呼ぶ現行実装は正規化される（ADR との乖離が解消される）
- タッチベースのイナーシャ・rubber-band は各 Platform Adapter が実装する責務となる
- `scroll-snap-type` / `scroll-snap-align` を Hayate CSS のサブセットとして定義し、Platform Adapter がスナップ計算に使う
- Hayabusa は scroll イベントをデフォルトでは購読しない。必要な場合のみ `onScroll` を使う
- `element_set_scroll_offset` はプログラマティックスクロール専用 API として意味が確定する
- ADR-0022 を破棄する
