# StylePatch/HayateStyle/EventKind を spec から生成し、'change' EventKind を撤去する

**Status: accepted**

**Date: 2026-06-10**

## Context

ADR-0055（wire codec）/ ADR-0070（DOM CSS mapping）により `codec.ts`（`encodeStylePatch` 等）と `catalog.ts`（DOM mapping）は `style_tags.json` から生成済みだが、Tsubame `renderer-protocol` の **`StylePatch`/`HayateStyle`（`style.ts`）と `EventKind`（`event.ts`）は手書きのまま spec と二重管理**だった（`docs/architecture-decisions-pending.md` 項目5）。

調査の過程で `EventKind` の `'change'` が**死んでいる/設計ミスの残骸**であることが判明した：

- `event_kinds.json` に対応する wire event が無く、Canvas Renderer では `HAYATE_LISTENER_KIND` に未登録のため `addEventListener(id, 'change', ...)` が no-op。
- DOM Renderer では `onChange` → `'change'` → ネイティブ `change` イベント（blur/commit 時のみ発火）にマップされていたが、これは React の `onChange`（毎入力で発火、実体は `input`）の慣習と矛盾する。
- 初出は最初期のスケルトン commit（`1cdf033`）で、Hayate 側の vocabulary と reconcile されないまま残っていた。

## Decision

1. **`'change'` EventKind を撤去**（`renderer-protocol/src/event.ts` の `EventKind`、`solid/src/events.ts` の `EVENT_PROP.onChange`、`renderer-dom/src/event-mapping.ts` の `DOM_EVENT_NAME`、`solid/src/jsx.ts` の `TsubameProps.onChange`）。これにより `EventKind` の値集合（7値）が `event_kinds.json` の `interactionKind` と完全一致する。
2. **codegen 範囲を拡張**：ADR-0055/0070 の延長として、`style_tags.json`（`name`+`encodeFrom`+`params`）から `StylePatch`/`HayateStyle` を、`event_kinds.json`（`interactionKind`）から `EventKind` を生成する。

## Considered Options

- **per-event-kind の payload 型生成**（`InteractionEvent` を discriminated union にし `payloadSchema` を spec に新設）：現状 `InteractionEvent` は `kind`+`target`+任意の `value?`/`key?` を持つ単一フラット interface であり、discriminated union を要求する既存利用箇所が無い。新規 spec フィールド設計を要する大きめの変更であり、既存の不便を解消するものでもないため**スコープ外**。
- **`change` を spec に追加し (b-2) 完全生成を維持**：`event_kinds.json` の `wireRole` は wire event のみを表す。`change` は DOM Renderer 固有の合成イベントであり、存在しない wire event を spec ででっち上げることになる。`change` 自体を撤去する方が spec の責務（wire vocabulary）を保てる。
- **`DomStylePatch`（DOM Z-order 警告 registry）の spec 同期**：機械的に導出できる入力源が spec 側に無く、手書き運用を継続（codegen 対象外として明記）。

## Consequences

- `StylePatch`/`HayateStyle`/`EventKind` が spec 駆動で生成され、`codec.ts`/`catalog.ts` と並ぶ二重管理解消の対象になる。
- `EventKind` から `'change'` が恒久的に消える。将来 DOM の `change` セマンティクスが必要になった場合は、まず `event_kinds.json` に実体のある wire event として追加することが前提になる。
- per-event payload 型・`DomStylePatch` 同期は引き続き手書き（明示的にスコープ外）。

## 関係

- `docs/architecture-decisions-pending.md` 項目5を解決。
- ADR-0055：wire codec 単一正本（`codec.ts` 生成の前例）。
- ADR-0070：DOM CSS mapping 単一正本（`catalog.ts` 生成の前例）。
