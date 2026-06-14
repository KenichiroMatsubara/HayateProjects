---
status: accepted
---

# ポインタ低レベル + ドラッグ高レベルの二層イベントモデル（設計確定・実装は後続）

ドラッグ並べ替え等のジェスチャを将来サポートするため、Interaction Event 語彙を二層で拡張する設計を確定する。**実装は本件では行わない**（デモ側は当面 up/down ボタンで並べ替える）が、後続作業が設計を再議論せずに進められるよう、契約形だけをここで固定する。

- **低レベル（pointer プリミティブ）**: `pointer-down` / `pointer-move` / `pointer-up` を座標付きで追加し、pointer capture を持つ。カスタムジェスチャ用の汎用基盤。
- **高レベル（DnD）**: `drag-start` / `drag-over` / `drag-drop` / `drag-end` を追加する。`drag-over` / `drag-drop` は drop 先解決済みで、対象 ElementId と挿入位置 `position`（`before` / `after` / `on`）を運ぶ。挿入位置は Hayate がポインタと対象中点の比較で算出する。

## 決定の要点

- **高レベルも spec（proto 契約）に含める** — JS 側ヘルパーに閉じず、契約に入れる。したがって DOM Renderer と Canvas Renderer（Hayate core dispatch）の両方が実装し、意味論パリティの対象になる。drop 先 hit-test は Hayate 内部で解決するため、Canvas 経路で JS 側が WASM 境界を越えて幾何を問い合わせる必要がない。
- **高レベルは低レベルの上に構築** — Hayate core 内で pointer プリミティブから DnD を導出し、両層をアプリが購読できる。
- **opt-in は element property** — `draggable` / `dropTarget` を既存の `disabled` / `src` と同様の typed property として宣言する（style や購読有無での暗黙化はしない）。レンダラー横断で明確になり、Hayate が hit-test 対象を絞れる。
- **payload は最小限** — `drag-start` / `drag-drop` は source / target / position のみ。HTML DnD の `dataTransfer` 相当（任意 MIME データ枠）は当面持たない。

## Considered Options

- **低レベルのみ + 高レベルは JS ヘルパー** — 契約は最小だが、Canvas 経路で drop 先 hit-test のために JS↔WASM 同期クエリ（または別途 hit-test API）が必要になり、DnD 意味論がレンダラーごとに割れるリスクがある。却下。
- **高レベル DnD のみ** — カスタムジェスチャの自由度を失う。却下。
- **二層とも契約に含める（採用）** — 契約面は広がるが、DnD 意味論を一箇所（Hayate core）に集約でき、パリティを保ったまま低レベルの拡張性も残せる。

## Consequences

- `proto/spec/event_kinds.json` への追加と `proto/spec` の element property 追加が必要（実装フェーズで）。
- 実装着手時はこの ADR の語彙・payload・position 算出規則を正本とする。
